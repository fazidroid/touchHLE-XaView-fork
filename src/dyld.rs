/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Dynamic linker.
//!
//! iPhone OS's dynamic linker, `dyld`, is the namesake of this module.
//!
//! This is where the magic of "high-level emulation" can begin to happen.
//! The guest app will reference various functions, constants, classes etc from
//! iPhone OS's system frameworks and other dynamically-linked libraries, but
//! instead of actually loading and linking the original framework binaries,
//! this "dynamic linker" will generate appropriate stubs for calling into
//! touchHLE's own implementations of the frameworks, which are "host code"
//! (i.e. not themselves running under emulation).
//!
//! This also does normal dynamic linking for libgcc, libstdc++, etc.
//!
//! See [crate::mach_o] for resources.

mod dylib_list;

use crate::abi::{CallFromGuest, GuestFunction};
use crate::cpu::Cpu;
use crate::frameworks::foundation::ns_string;
use crate::mach_o::{MachO, SectionType};
use crate::mem::{ConstVoidPtr, GuestUSize, Mem, MutPtr, Ptr};
use crate::objc::{nil, ClassExports, ObjC};
use crate::Environment;
use std::collections::HashMap;

pub use dylib_list::DYLIB_LIST;

/// Struct used to expose a host implementation of a dynamic library (usually a
/// framework) to the linker.
///
/// Each module that wants to expose a library to guest code should export a
/// constant using this type, which collects all the relevant [ClassExports],
/// [ConstantExports] and [FunctionExports] for the library. For example:
///
/// ```ignore
/// pub const DYLIB: HostDylib = HostDylib {
///     path: "/System/Library/Frameworks/FooBarKit.framework/FooBarKit",
///     aliases: &[],
///     class_exports: &[baz::CLASSES],
///     constant_exports: &[qux::CONSTANTS],
///     function_exports: &[qux::FUNCTIONS, baz::FUNCTIONS],
/// };
/// ```
///
/// The `path` should be the canonical notional filesystem path that the library
/// is referenced by on the real OS, for example `"/usr/lib/libobjc.A.dylib"`
/// or `"/System/Library/Frameworks/Foundation.framework/Foundation"`. For
/// libraries that have several symlinked paths, non-canonical alternate
/// paths can be listed under `aliases`, for example `"/usr/lib/libobjc.dylib"`.
pub struct HostDylib {
    pub path: &'static str,
    pub aliases: &'static [&'static str],
    pub class_exports: &'static [ClassExports],
    pub constant_exports: &'static [ConstantExports],
    pub function_exports: &'static [FunctionExports],
}

pub type HostFunction = &'static dyn CallFromGuest;

/// Type for lists of functions exported by host implementations of dynamic
/// libraries (usually frameworks).
///
/// Each module that wants to expose functions to guest code should export a
/// constant using this type, e.g.:
///
/// ```ignore
/// pub const FUNCTIONS: FunctionExports = &[
///    ("_NSFoo", &/* ... */),
///    ("_NSBar", &/* ... */),
///    /* ... */
/// ];
/// ```
///
/// All the constants like this can then be collected into a [HostDylib].
///
/// The strings are the mangled symbol names. For C functions, this is just the
/// name prefixed with an underscore.
///
/// For convenience, use [export_c_func]:
///
/// ```ignore
/// pub const FUNCTIONS: FunctionExports = &[
///     export_c_func!(NSFoo(_, _)),
///     export_c_func!(NSBar()),
/// ];
/// ```
///
/// See also [ConstantExports] and [ClassExports].
pub type FunctionExports = &'static [(&'static str, HostFunction)];

/// Macro for exporting a function with C-style name mangling. See
/// [FunctionExports].
///
/// ```ignore
/// export_c_func!(NSFoo(_, _))
/// ```
///
/// will desugar to:
///
/// ```ignore
/// ("_NSFoo", &(NSFoo as (&mut Environment, _, _) -> _))
/// ```
///
/// The function needs to be explicitly casted because a bare function reference
/// defaults to a different type than a pure fn pointer, which is the type that
/// [CallFromGuest] is implemented on. This macro will do the casting for you,
/// but you will need to supply an underscore for each parameter.
#[macro_export]
macro_rules! export_c_func {
    ($name:ident ($($_:ty),*)) => {
        (
            concat!("_", stringify!($name)),
            &($name as fn(&mut $crate::Environment, $($_),*) -> _)
        )
    };
}
pub use crate::export_c_func; // #[macro_export] is weird...

/// Other variant of [export_c_func] macro, allowing to define an alias
/// for the exporting function. This is useful then alias may contain
/// characters not normally allowed for Rust function's names. (e.g. `$`)
#[macro_export]
macro_rules! export_c_func_aliased {
    ($alias:literal, $name:ident ($($_:ty),*)) => {
        (
            concat!("_", $alias),
            &($name as fn(&mut $crate::Environment, $($_),*) -> _)
        )
    };
}
pub use crate::export_c_func_aliased; // #[macro_export] is weird...

/// Type for describing a constant (C `extern const` symbol) that will be
/// created by the linker if the guest app references it. See [ConstantExports].
pub enum HostConstant {
    NSString(&'static str),
    NullPtr,
    Custom(fn(&mut Environment) -> ConstVoidPtr),
    Bytes(&'static [u8]), // RawBytesVariant
}

/// Type for lists of constants exported by host implementations of  dynamic
/// libraries (usually frameworks).
///
/// Each module that wants to expose functions to guest code should export a
/// constant using this type, e.g.:
///
/// ```ignore
/// pub const CONSTANT: ConstantExports = &[
///    ("_kNSFooBar", HostConstant::NSString("NSFooBar")),
///    /* ... */
/// ];
/// ```
///
/// All the constants like this can then be collected into a [HostDylib].
///
/// The strings are the mangled symbol names. For C constants, this is just the
/// name prefixed with an underscore.
///
/// See also [FunctionExports], [ClassExports].
pub type ConstantExports = &'static [(&'static str, HostConstant)];

/// Search the list of [HostDylib]s for a class/constant/function by its symbol.
///
/// Example usage: `search_host_dylibs(|dylib| dylib.function_exports, "_foo")`
pub fn search_host_dylibs<T, F>(get_exports: F, symbol: &str) -> Option<&'static (&'static str, T)>
where
    F: Fn(&HostDylib) -> &'static [&'static [(&'static str, T)]],
{
    // TODO: In general, we should rarely if ever need to search the full set
    //       of dylibs for a symbol. Now that we know which symbols belong to
    //       which libraries, we should at least only search libraries that are
    //       referenced by the app and currently "loaded". We probably should
    //       also implement the Mach-O two-level symbol namespacing eventually.
    DYLIB_LIST
        .iter()
        .copied()
        .map(get_exports)
        .find_map(|lists| search_lists(lists, symbol))
}

/// Helper for working with [ClassExports]/[ConstantExports]/[FunctionExports].
fn search_lists<T>(
    lists: &'static [&'static [(&'static str, T)]],
    symbol: &str,
) -> Option<&'static (&'static str, T)> {
    lists
        .iter()
        .flat_map(|&n| n)
        .find(|&(sym, _)| *sym == symbol)
}

fn encode_a32_svc(imm: u32) -> u32 {
    assert!(imm & 0xff000000 == 0);
    imm | 0xef000000
}
fn encode_a32_ret() -> u32 {
    0xe12fff1e
}
fn encode_a32_trap() -> u32 {
    0xe7ffdefe
}

fn write_return_to_host_routine(mem: &mut Mem, svc: u32) -> GuestFunction {
    let routine = [
        encode_a32_svc(svc),
        // When a return-to-host occurs, it's the host's responsibility
        // to reset the PC to somewhere else. So something has gone
        // wrong if this is executed.
        encode_a32_trap(),
    ];
    let ptr: MutPtr<u32> = mem.alloc(4 * 2).cast();
    mem.write(ptr + 0, routine[0]);
    mem.write(ptr + 1, routine[1]);
    let ptr = GuestFunction::from_addr_with_thumb_bit(ptr.to_bits());
    assert!(!ptr.is_thumb());
    ptr
}
pub struct Dyld {
    pub is_64_bit: bool, // 🏎️ ADD THIS FLAG
    linked_host_functions: Vec<(&'static str, HostFunction)>,
    return_to_host_routine: Option<GuestFunction>,
    thread_exit_routine: Option<GuestFunction>,
    constants_to_link_later: Vec<(MutPtr<ConstVoidPtr>, &'static HostConstant)>,
    non_lazy_host_functions: HashMap<&'static str, GuestFunction>,
}

impl Dyld {
    /// We reserve this SVC ID for invoking the lazy linker.
    pub const SVC_LAZY_LINK: u32 = 0;
    /// We reserve this SVC ID for the exit routine for spawned threads.
    pub const SVC_THREAD_EXIT: u32 = 1;
    /// We reserve this SVC ID for the special return-to-host routine.
    pub const SVC_RETURN_TO_HOST: u32 = 2;
    /// The range of SVC IDs `SVC_LINKED_FUNCTIONS_BASE..` is used to reference
    /// [Self::linked_host_functions] entries.
    pub const SVC_LINKED_FUNCTIONS_BASE: u32 = Self::SVC_RETURN_TO_HOST + 1;
    /// We reserve this SVC ID for lazy linking and returning right after.
    /// It is also a mask for the linked functions to indicate that an
    /// additional return instruction needs to be manually executed after
    /// handling the SVC.
    pub const SVC_LAZY_LINK_RET_FLAG: u32 = 0x800000;

    const SYMBOL_STUB1_INSTRUCTIONS: [u32; 1] = [0xe59ff000]; // mask this with lowest 12 bits to restore instructions
    const SYMBOL_STUB_INSTRUCTIONS: [u32; 2] = [0xe59fc000, 0xe59cf000];
    const PIC_SYMBOL_STUB_INSTRUCTIONS: [u32; 3] = [0xe59fc004, 0xe08fc00c, 0xe59cf000];

    pub fn new() -> Dyld {
        Dyld {
            is_64_bit: false, // 🏎️ INITIALIZE IT TO FALSE
            linked_host_functions: Vec::new(),
            return_to_host_routine: None,
            thread_exit_routine: None,
            constants_to_link_later: Vec::new(),
            non_lazy_host_functions: HashMap::new(),
        }
    }
    pub fn return_to_host_routine(&self) -> GuestFunction {
        self.return_to_host_routine.unwrap()
    }

    pub fn thread_exit_routine(&self) -> GuestFunction {
        self.thread_exit_routine.unwrap()
    }

    /// Do linking-related tasks that need doing right after loading the
    /// binaries.
    pub fn do_initial_linking(&mut self, bins: &[MachO], mem: &mut Mem, objc: &mut ObjC) {
        assert!(self.return_to_host_routine.is_none());
        assert!(self.thread_exit_routine.is_none());
        self.return_to_host_routine =
            Some(write_return_to_host_routine(mem, Self::SVC_RETURN_TO_HOST));
        self.thread_exit_routine = Some(write_return_to_host_routine(mem, Self::SVC_THREAD_EXIT));

        // Currently assuming only the app binary contains Objective-C things.

        objc.register_bin_selectors(&bins[0], mem);
        objc.register_host_selectors(mem);

        for bin in bins {
            self.setup_lazy_linking(bin, mem);
            // Must happen before `register_bin_classes`, else superclass
            // pointers will be wrong.
            self.do_non_lazy_linking(bin, bins, mem, objc);
        }

        objc.register_bin_classes(&bins[0], mem);
        objc.register_bin_categories(&bins[0], mem);

        ns_string::register_constant_strings(&bins[0], mem, objc);
    }

    /// Dumps all lazy symbols (functions) referenced by the binary
    /// as JSON to stdout.
    ///
    /// The JSON has the following form:
    /// ```json
    /// {
    ///     "object": "lazy_symbols",
    ///     "symbols": [
    ///         {
    ///             "symbol": ((name of symbol)),
    ///             "linked_to": "host" | "dylib" | null,
    ///             "dylib": ((name of dylib)) | null,
    ///         },
    ///         ...
    ///     ]
    /// }
    /// ```
    pub fn dump_lazy_symbols(
        &mut self,
        bins: &[MachO],
        file: &mut std::fs::File,
    ) -> Result<(), std::io::Error> {
        use std::io::Write;
        // Guest binary is always bin 0.
        let stubs = bins[0].get_section(SectionType::SymbolStubs).unwrap();
        let info = stubs.dyld_indirect_symbol_info.as_ref().unwrap();
        writeln!(
            file,
            "{{\n    \"object\":\"lazy_symbols\",\n    \"symbols\": ["
        )?;

        'sym: for (i, symbol) in info.indirect_undef_symbols.iter().enumerate() {
            // Why doesn't json allow trailing commas...
            let comma = if i == info.indirect_undef_symbols.len() - 1 {
                ""
            } else {
                ","
            };
            let symbol = symbol.as_ref().unwrap();
            if let Some(&(_, _)) = search_host_dylibs(|dylib| dylib.function_exports, symbol) {
                writeln!(
                    file,
                    "        {{ \"symbol\": \"{symbol}\", \"linked_to\": \"host\"}}{comma}"
                )?;
                continue;
            }
            for dylib in bins.iter() {
                if dylib.exported_symbols.contains_key(symbol) {
                    writeln!(
                        file,
                        "        {{ \"symbol\": \"{}\", \"linked_to\": \"dylib\", \"dylib\": \"{}\"}}{}",
                        symbol, dylib.name, comma
                    )?;
                    continue 'sym;
                }
            }
            writeln!(file, "        {{ \"symbol\": \"{symbol}\" }}{comma}")?;
        }
        writeln!(file, "    ]\n}}")
    }

    /// Dumps all non-objc symbols provided by touchHLE.
    ///
    /// The dump format is Objective-C code (with meaningless types) that can be
    /// compiled to generate stub libraries that can be linked against, with
    /// comments providing the paths each library would be installed to.
    /// This is used for building the integration tests.
    pub fn dump_host_symbols(file: &mut std::fs::File) -> Result<(), std::io::Error> {
        use std::io::Write;
        for dylib in DYLIB_LIST {
            writeln!(file, "// {}", dylib.path)?;
            for alias in dylib.aliases {
                writeln!(file, "// {alias}")?;
            }
            for (class_name, _) in dylib.class_exports.iter().copied().flatten() {
                writeln!(file, "@interface {class_name}")?;
                writeln!(file, "@end")?;
                writeln!(file, "@implementation {class_name}")?;
                writeln!(file, "@end")?;
            }
            for (constant_symbol, _) in dylib.constant_exports.iter().copied().flatten() {
                writeln!(file, "int {};", constant_symbol.strip_prefix("_").unwrap())?;
            }
            for (function_symbol, _) in dylib.function_exports.iter().copied().flatten() {
                writeln!(
                    file,
                    "void {}() {{}}",
                    function_symbol.strip_prefix("_").unwrap()
                )?;
            }
        }
        Ok(())
    }

    /// [Self::do_initial_linking] but for when this is the app picker's special
    /// environment with no binary (see [crate::Environment::new_without_app]).
    pub fn do_initial_linking_with_no_bins(&mut self, mem: &mut Mem, objc: &mut ObjC) {
        assert!(self.return_to_host_routine.is_none());
        assert!(self.thread_exit_routine.is_none());
        self.return_to_host_routine =
            Some(write_return_to_host_routine(mem, Self::SVC_RETURN_TO_HOST));
        self.thread_exit_routine = Some(write_return_to_host_routine(mem, Self::SVC_THREAD_EXIT));

        objc.register_host_selectors(mem);
    }

    /// Set up lazy-linking stubs for a loaded binary.
    ///
    /// Dynamic linking of functions on iPhone OS usually happens "lazily",
    /// which means that the linking is delayed until the function is first
    /// called. This is achieved by using stub functions. Instead of calling the
    /// external function directly, the app code will call a stub function, and
    /// that stub will either jump to the dynamic linker (which will link in the
    /// external function and then jump to it), or on subsequent calls, jump
    /// straight to the external function.
    ///
    /// These stubs already exist in the binary, but they need to be rewritten
    /// so that they will invoke our dynamic linker.
    fn setup_lazy_linking(&self, bin: &MachO, mem: &mut Mem) {
        let Some(stubs) = bin.get_section(SectionType::SymbolStubs) else {
            return;
        };

        let entry_size = stubs.dyld_indirect_symbol_info.as_ref().unwrap().entry_size;

        // two or three A32 instructions (PIC stub needs one more) followed by
        // the address or offset of the corresponding __la_symbol_ptr
        let expected_instructions = match entry_size {
            4 => &[],
            12 => Self::SYMBOL_STUB_INSTRUCTIONS.as_slice(),
            16 => Self::PIC_SYMBOL_STUB_INSTRUCTIONS.as_slice(),
            _ => unimplemented!(),
        };

        assert!(stubs.size % entry_size == 0);
        let stub_count = stubs.size / entry_size;
        for i in 0..stub_count {
            let ptr: MutPtr<u32> = Ptr::from_bits(stubs.addr + i * entry_size);

            for (j, &instr) in expected_instructions.iter().enumerate() {
                assert!(mem.read(ptr + j.try_into().unwrap()) == instr);
            }

            // For convenience, make the stub return once the SVC is done
            // (Otherwise we have to manually update the PC)
            if entry_size == 4 {
                mem.write(ptr + 0, encode_a32_svc(Self::SVC_LAZY_LINK_RET_FLAG));
            } else {
                mem.write(ptr + 0, encode_a32_svc(Self::SVC_LAZY_LINK));
                mem.write(ptr + 1, encode_a32_ret());
            }
            if entry_size == 16 {
                // This is preceded by a return instruction, so if we do execute
                // it, something has gone wrong.
                mem.write(ptr + 2, encode_a32_trap());
            }
            // Leave the __la_symbol_ptr intact in case we want to link it to
            // a real symbol later.
        }
    }

    /// Link non-lazy symbols for a loaded binary.
    ///
    /// These are usually constants, Objective-C classes, or vtable pointers.
    /// Since the linking must be done upfront, we can't in general delay errors
    /// about missing implementations until the point of use. For that reason,
    /// this will spit out a warning to stderr for everything missing, so that
    /// there's at least some indication about why the emulator might crash.
    ///
    /// `bin` is the binary to link non-lazy symbols for, `bins` is the set of
    /// binaries symbols may be looked up in.
    fn do_non_lazy_linking(&mut self, bin: &MachO, bins: &[MachO], mem: &mut Mem, objc: &mut ObjC) {
        let mut unhandled_relocations: HashMap<&str, Vec<u32>> = HashMap::new();
        for &(ptr_ptr, ref name) in &bin.external_relocations {
            let ptr_ptr: MutPtr<ConstVoidPtr> = Ptr::from_bits(ptr_ptr);
            // There will be an existing value at the address, which is an
            // offset that should be applied to the external symbol's address.
            // It is often 0, but not always.
            let offset: u32 = mem.read(ptr_ptr).to_bits();
            let target: ConstVoidPtr = if let Some(name) = name.strip_prefix("_OBJC_CLASS_$_") {
                objc.link_class(name, /* is_metaclass: */ false, mem)
                    .cast()
                    .cast_const()
            } else if let Some(name) = name.strip_prefix("_OBJC_METACLASS_$_") {
                objc.link_class(name, /* is_metaclass: */ true, mem)
                    .cast()
                    .cast_const()
            } else if name == "___CFConstantStringClassReference" {
                // See ns_string::register_constant_strings
                nil.cast().cast_const()
            } else if let Some(&external_addr) = bins
                .iter()
                .flat_map(|other_bin| other_bin.exported_symbols.get(name))
                .next()
            {
                // Often used for C++ RTTI
                Ptr::from_bits(external_addr)
            } else if let Some((symbol, _)) =
                search_host_dylibs(|dylib| dylib.function_exports, name)
            {
                // We want the same symbol name to always point to the same
                // function.
                let trampoline_ptr = self
                    .create_proc_address_no_inval(mem, symbol)
                    .unwrap()
                    .to_ptr();
                log_dbg!(
                    "Linked external relocation to host function {} at {:?}",
                    symbol,
                    trampoline_ptr
                );
                trampoline_ptr
            } else if search_host_dylibs(|dylib| dylib.constant_exports, name).is_some() {
                // Skip the constants from DYLD_INFO because we already
                // handle the consts when reading the __nl_symbol_ptr section
                continue;
            } else {
                unhandled_relocations
                    .entry(name)
                    .or_default()
                    .push(ptr_ptr.to_bits());
                continue;
            };
            // wrapping_add() is used in case the offset is negative. I haven't
            // seen it happen, but it would make sense if that is allowed.
            mem.write(
                ptr_ptr,
                Ptr::from_bits(target.to_bits().wrapping_add(offset)),
            )
        }
        // Collecting unhandled relocations for the same symbol onto one line
        // makes the log output much less spammy.
        for (name, addrs) in unhandled_relocations {
            log!(
                "Warning: unhandled external relocation {:?} in {:?} at {}",
                name,
                bin.name,
                addrs
                    .into_iter()
                    .map(|addr| format!("{addr:#x}"))
                    .collect::<Vec<String>>()
                    .join(", "),
            );
        }

        let Some(ptrs) = bin.get_section(SectionType::NonLazySymbolPointers) else {
            return;
        };
        let info = ptrs.dyld_indirect_symbol_info.as_ref().unwrap();

        let entry_size = info.entry_size;
        assert!(entry_size == 4);
        assert!(ptrs.size % entry_size == 0);
        let ptr_count = ptrs.size / entry_size;
        'ptr_loop: for i in 0..ptr_count {
            let Some(symbol) = info.indirect_undef_symbols[i as usize].as_deref() else {
                continue;
            };

            let ptr_ptr: MutPtr<ConstVoidPtr> = Ptr::from_bits(ptrs.addr + i * entry_size);

            for other_bin in bins {
                if let Some(&addr) = other_bin.exported_symbols.get(symbol) {
                    mem.write(ptr_ptr, Ptr::from_bits(addr));
                    continue 'ptr_loop;
                }
            }

            if let Some((symbol, _)) = search_host_dylibs(|dylib| dylib.function_exports, symbol) {
                // We want the same symbol name to always point to the same
                // function. It could point to a specific stub entry, but it's
                // easier to just create a new function and point all the stub
                // entries to it.
                let trampoline_ptr = self
                    .create_proc_address_no_inval(mem, symbol)
                    .unwrap()
                    .to_ptr();
                mem.write(ptr_ptr, trampoline_ptr);
                log_dbg!(
                    "Linked non-lazy host function {} at {:?}",
                    symbol,
                    trampoline_ptr
                );
                log_dbg!("{:?}", self.non_lazy_host_functions);
                continue;
            }
            if let Some((_, template)) = search_host_dylibs(|dylib| dylib.constant_exports, symbol)
            {
                // Delay linking of constant until we have a `&mut Environment`,
                // that makes it much easier to build NSString objects etc.
                self.constants_to_link_later.push((ptr_ptr, template));
                continue;
            }

            // ___stack_chk_guard must be a non-zero sentinel or any
            // stack-protected function will silently corrupt or crash.
            // On real iOS this is randomized per-boot; a fixed value is fine
            // for emulation — we only need it non-zero so the canary check
            // doesn't immediately trigger.
            if symbol == "___stack_chk_guard" {
                let canary: u32 = 0xDEAD_C0DE;
                let canary_ptr = mem.alloc_and_write(canary);
                mem.write(ptr_ptr, canary_ptr.cast().cast_const());
                log_dbg!("Initialized ___stack_chk_guard sentinel at {:?}", canary_ptr);
                continue;
            }

            log!(
                "Warning: unhandled non-lazy symbol {:?} at {:?} in \"{}\"",
                symbol,
                ptr_ptr,
                bin.name
            );
        }

        // FIXME: check for internal relocations?
    }

    /// Do linking that can only be done once there is a full [Environment].
    /// Not to be confused with lazy linking.
    pub fn do_late_linking(env: &mut Environment) {
        // TODO: do symbols ever appear in __nl_symbol_ptr multiple times?

        let to_link = std::mem::take(&mut env.dyld.constants_to_link_later);
        for (symbol_ptr_ptr, template) in to_link {
            let symbol_ptr: ConstVoidPtr = match template {
                HostConstant::NSString(static_str) => {
                    let string_ptr = ns_string::get_static_str(env, static_str);
                    let string_ptr_ptr = env.mem.alloc_and_write(string_ptr);
                    string_ptr_ptr.cast().cast_const()
                }
                HostConstant::NullPtr => {
                    let null_ptr: ConstVoidPtr = Ptr::null();
                    let null_ptr_ptr = env.mem.alloc_and_write(null_ptr);
                    null_ptr_ptr.cast().cast_const()
                }
                HostConstant::Custom(f) => f(env),
                HostConstant::Bytes(bytes) => {
                    let ptr = env.mem.alloc(bytes.len() as u32);
                    for (i, &b) in bytes.iter().enumerate() {
                        env.mem.write(ptr.cast::<u8>() + (i as u32), b);
                    }
                    ptr.cast_const()
                }
            };
            env.mem.write(symbol_ptr_ptr, symbol_ptr.cast());
        }
    }

    /// Return a host function that can be called to handle an SVC instruction
    /// encountered during CPU emulation. If `None` is returned, the execution
    /// needs to resume at `svc_pc`.
    pub fn get_svc_handler(
        &mut self,
        bins: &[MachO],
        mem: &mut Mem,
        cpu: &mut Cpu,
        svc_pc: u32,
        svc: u32,
    ) -> Option<HostFunction> {
        match svc {
            Self::SVC_LAZY_LINK | Self::SVC_LAZY_LINK_RET_FLAG => {
                self.do_lazy_link(bins, mem, cpu, svc_pc)
            }
            Self::SVC_THREAD_EXIT | Self::SVC_RETURN_TO_HOST => unreachable!(), // don't handle here
            Self::SVC_LINKED_FUNCTIONS_BASE.. => {
                let f = self.linked_host_functions.get(
                    ((svc & !Self::SVC_LAZY_LINK_RET_FLAG) - Self::SVC_LINKED_FUNCTIONS_BASE)
                        as usize,
                );
                let Some(&(symbol, f)) = f else {
                    // NullPageSvcBypass: the game jumped to a null/invalid
                    // pointer and hit garbage bytes that decoded as an SVC.
                    // Redirect to thread_exit_routine so the thread dies
                    // cleanly rather than panicking the whole emulator.
                    if svc_pc < 0x2000 {
                        echo!(
                            "WARNING: SVC #{} at null-page {:#x}, redirecting to thread_exit_routine.",
                            svc, svc_pc
                        );
                        cpu.branch(self.thread_exit_routine());
                        return None;
                    }
                    panic!("Unexpected SVC #{svc} at {svc_pc:#x}");
                };
                log_dbg!("Call to host function, already linked: {}", symbol);
                Some(f)
            }
        }
    }

    fn do_lazy_link(
        &mut self,
        bins: &[MachO],
        mem: &mut Mem,
        cpu: &mut Cpu,
        svc_pc: u32,
    ) -> Option<HostFunction> {
        // Links by restoring the original stub function, then updating
        // __la_symbol_ptr to the appropriate function.
        fn link_by_restoring_stub(
            mem: &mut Mem,
            cpu: &mut Cpu,
            linked_function: u32,
            svc_pc: u32,
            entry_size: u32,
            pic_offset: u32,
        ) -> (MutPtr<u32>, MutPtr<u32>) {
            let original_instructions = match entry_size {
                4 => Dyld::SYMBOL_STUB1_INSTRUCTIONS.as_slice(),
                12 => Dyld::SYMBOL_STUB_INSTRUCTIONS.as_slice(),
                16 => Dyld::PIC_SYMBOL_STUB_INSTRUCTIONS.as_slice(),
                _ => unreachable!(),
            };
            let instruction_count: GuestUSize = original_instructions.len().try_into().unwrap();

            // Restore the original stub, which calls the __la_symbol_ptr
            let stub_function_ptr: MutPtr<u32> = Ptr::from_bits(svc_pc);
            if entry_size == 4 {
                mem.write(stub_function_ptr, original_instructions[0] | pic_offset)
            } else {
                for (i, &instr) in original_instructions.iter().enumerate() {
                    mem.write(stub_function_ptr + i.try_into().unwrap(), instr)
                }
            }

            cpu.invalidate_cache_range(stub_function_ptr.to_bits(), instruction_count * 4);

            // Update the __la_symbol_ptr
            let la_symbol_ptr: MutPtr<u32> = if entry_size == 12 {
                // Normal stub: absolute address
                let addr = mem.read(stub_function_ptr + instruction_count);
                Ptr::from_bits(addr)
            } else {
                // The PIC (position-independent code) stub uses a
                // PC-relative offset rather than an absolute address.
                if entry_size == 4 {
                    let offset = mem.read(stub_function_ptr) & 0xFFF;
                    Ptr::from_bits(stub_function_ptr.to_bits() + offset + 8)
                } else {
                    let offset = mem.read(stub_function_ptr + instruction_count);
                    Ptr::from_bits(stub_function_ptr.to_bits() + offset + 12)
                }
            };
            mem.write(la_symbol_ptr, linked_function);
            (stub_function_ptr, la_symbol_ptr)
        }

        let Some((stubs, pic_offset)) = bins.iter().find_map(|bin| {
            let stubs = bin.get_section(SectionType::SymbolStubs)?;
            if !(stubs.addr..(stubs.addr + stubs.size)).contains(&svc_pc) {
                return None;
            }
            let pic_offset = bin
                .get_section(SectionType::LazySymbolPointers)
                .map_or(0, |lazy_ptrs| lazy_ptrs.addr - stubs.addr);
            Some((stubs, pic_offset))
        }) else {
            let r12 = cpu.regs()[12];
            let r0 = cpu.regs()[0];
            // LogInlineSvc
            log!(
                "WARNING: Unresolved inline SVC at {:#010x}! Syscall ID (R12): {}, R0: {:#x}",
                svc_pc,
                r12,
                r0
            );
            // SafeInlineSvc
            fn safe_fallback(env: &mut crate::Environment) {
                let r12 = env.cpu.regs()[12];
                if r12 == 10 {
                    let r1 = env.cpu.regs()[1];
                    let r2 = env.cpu.regs()[2];
                    let ptr = env.mem.alloc(r2).to_bits();
                    env.mem.write(crate::mem::MutPtr::<u32>::from_bits(r1), ptr);
                    env.cpu.regs_mut()[0] = 0;
                    println!(
                        "WARNING: Inline mach_vm_allocate size: {:#x} -> {:#x}",
                        r2, ptr
                    );
                    return;
                }
                env.cpu.regs_mut()[0] = 0;
            }
            return Some(&(safe_fallback as fn(&mut crate::Environment) -> ()));
        };

        let info = stubs.dyld_indirect_symbol_info.as_ref().unwrap();

        let offset = svc_pc - stubs.addr;
        assert!(offset.is_multiple_of(info.entry_size));
        let idx = (offset / info.entry_size) as usize;

        let symbol = info.indirect_undef_symbols[idx].as_deref().unwrap();

        if let Some(&addr) = self.non_lazy_host_functions.get(symbol) {
            // The host function was already linked non-lazily, point the
            // stub and __la_symbol_ptr to the function.
            let (stub_function_ptr, la_symbol_ptr) = link_by_restoring_stub(
                mem,
                cpu,
                addr.addr_with_thumb_bit(),
                svc_pc,
                info.entry_size,
                pic_offset,
            );
            log_dbg!(
                "Linked host function {} at {:?}/{:?} to existing stub ({:?}).",
                symbol,
                stub_function_ptr,
                la_symbol_ptr,
                addr,
            );
            // The stub jumps to the non-lazy function, which calls the
            // host function.
            return None;
        }

        if let Some(&(symbol, f)) = search_host_dylibs(|dylib| dylib.function_exports, symbol) {
            // Allocate an SVC ID for this host function
            let idx: u32 = self.linked_host_functions.len().try_into().unwrap();
            let mut svc = idx + Self::SVC_LINKED_FUNCTIONS_BASE;
            // Indicate to the handler to return manually after call
            if info.entry_size == 4 {
                assert!(svc < Self::SVC_LAZY_LINK_RET_FLAG);
                svc |= Self::SVC_LAZY_LINK_RET_FLAG;
            }
            self.linked_host_functions.push((symbol, f));

            // Rewrite stub function to call this host function
            let stub_function_ptr: MutPtr<u32> = Ptr::from_bits(svc_pc);
            mem.write(stub_function_ptr, encode_a32_svc(svc));
            if info.entry_size != 4 {
                assert!(mem.read(stub_function_ptr + 1) == encode_a32_ret());
            }

            cpu.invalidate_cache_range(stub_function_ptr.to_bits(), 4);

            log_dbg!(
                "Linked {} at {:?} to host implementation",
                symbol,
                stub_function_ptr
            );

            // Return the host function so that we can call it now that we're
            // done.
            return Some(f);
        }

        for dylib in bins.iter() {
            if let Some(&addr) = dylib.exported_symbols.get(symbol) {
                let (stub_function_ptr, la_symbol_ptr) =
                    link_by_restoring_stub(mem, cpu, addr, svc_pc, info.entry_size, pic_offset);
                log_dbg!(
                    "Linked {} at {:?}/{:?} to {:#x} from {}",
                    symbol,
                    stub_function_ptr,
                    la_symbol_ptr,
                    addr,
                    dylib.name
                );
                // Tell the caller it needs to restart execution at svc_pc.
                return None;
            }
        }
        // ==========================================================
        // 🏎️ THE DNS SINKHOLE: Kill EA/Gameloft Telemetry Deadlocks
        // ==========================================================
        if symbol == "_gethostbyname" {
            fn fake_gethostbyname(env: &mut crate::Environment, name_ptr: u32) -> u32 {
                let name = if name_ptr != 0 {
                    env.mem.cstr_at_utf8(crate::mem::ConstPtr::<u8>::from_bits(name_ptr)).unwrap_or("unknown")
                } else {
                    "null"
                };
                println!("🎮 LOG: DNS Sinkholed gethostbyname request to: {}", name);
                0 // Return NULL to instantly fail the connection!
            }
            return Some(&(fake_gethostbyname as fn(&mut crate::Environment, u32) -> u32));
        }

        if symbol == "_getaddrinfo" {
            fn fake_getaddrinfo(env: &mut crate::Environment, node: u32, _service: u32, _hints: u32, _res: u32) -> i32 {
                let name = if node != 0 {
                    env.mem.cstr_at_utf8(crate::mem::ConstPtr::<u8>::from_bits(node)).unwrap_or("unknown")
                } else {
                    "null"
                };
                println!("🎮 LOG: DNS Sinkholed getaddrinfo request to: {}", name);
                8 // EAI_NONAME (nodename nor servname provided, or not known)
            }
            return Some(&(fake_getaddrinfo as fn(&mut crate::Environment, u32, u32, u32, u32) -> i32));
        }

        if symbol == "_connect" {
            fn fake_connect(_env: &mut crate::Environment, _socket: i32, _addr: u32, _len: u32) -> i32 {
                println!("🎮 LOG: Aggressively blocked socket connect() attempt!");
                -1 // -1 = Connection Refused
            }
            return Some(&(fake_connect as fn(&mut crate::Environment, i32, u32, u32) -> i32));
        }

        // ==========================================================
        // 🏎️ KEYCHAIN BYPASS: Prevent Garbage Pointer Crashes!
        // ==========================================================
        if symbol == "_SecItemCopyMatching" || symbol == "_SecItemAdd" || 
           symbol == "_SecItemUpdate" || symbol == "_SecItemDelete" {
            
            // We use a generic 2-argument stub. If the game passes fewer, AArch64/ARMv7 
            // safely ignores the extra register, preventing stack corruption!
            fn fake_sec_item(_env: &mut crate::Environment, _query: u32, _result: u32) -> i32 {
                println!("🎮 LOG: Safely blocked iOS Keychain request! Returning errSecItemNotFound.");
                
                // -25300 is the official iOS error code for 'errSecItemNotFound'.
                // This forces the game to gracefully build a new save file instead 
                // of trying to read uninitialized garbage memory!
                -25300 
            }
            return Some(&(fake_sec_item as fn(&mut crate::Environment, u32, u32) -> i32));
        }

        // ==========================================================
        // 🏎️ GT RACING 2 BYPASS: Stub __dyld_image_count
        // ==========================================================
        if symbol == "__dyld_image_count" {
            fn fake_dyld_image_count(_env: &mut crate::Environment) -> u32 {
                println!("🎮 LOG: Bypassing __dyld_image_count (Jailbreak/Anti-cheat check)!");
                // Returning 0 forces the game to skip its library-scanning loop entirely
                0 
            }
            return Some(&(fake_dyld_image_count as fn(&mut crate::Environment) -> u32));
        } 

        // ==========================================================
        // 🏎️ GT RACING 2 BYPASS: Stub _host_info
        // ==========================================================
        if symbol == "_host_info" {
            fn fake_host_info(
                env: &mut crate::Environment,
                host: u32,
                flavor: i32,
                host_info_out: crate::mem::MutPtr<u32>,
                host_info_out_cnt: crate::mem::MutPtr<u32>,
            ) -> i32 {
                log!("_host_info called (host={}, flavor={})", host, flavor);
                // HOST_BASIC_INFO = 1, HOST_SCHED_INFO = 3, etc.
                // We'll return a minimal HOST_BASIC_INFO structure.
                match flavor {
                    1 => { // HOST_BASIC_INFO
                        // Structure: max_cpus (i32), avail_cpus (i32), memory_size (u32), cpu_type (u32), cpu_subtype (u32)
                        let basic_info: [u32; 5] = [1, 1, 512 * 1024 * 1024, 12, 0];
                        let count = env.mem.read(host_info_out_cnt);
                        let copy_len = count.min(basic_info.len() as u32);
                        for i in 0..copy_len {
                            env.mem.write(host_info_out + i, basic_info[i as usize]);
                        }
                        env.mem.write(host_info_out_cnt, copy_len);
                        0 // KERN_SUCCESS
                    }
                    _ => {
                        log!("_host_info flavor {} unimplemented, returning KERN_INVALID_ARGUMENT", flavor);
                        4 // KERN_INVALID_ARGUMENT
                    }
                }
            }
            return Some(
                &(fake_host_info
                    as fn(
                        &mut crate::Environment,
                        u32,
                        i32,
                        crate::mem::MutPtr<u32>,
                        crate::mem::MutPtr<u32>,
                    ) -> i32),
            );
        }

                // FakeForkCall
        if symbol == "_fork" {
            fn fake_fork(env: &mut crate::Environment) {
                env.cpu.regs_mut()[0] = 0xffffffff;
            }
            return Some(&(fake_fork as fn(&mut crate::Environment) -> ()));
        }
                // ==========================================================
        // 🏎️ REAL RACING 2 BYPASS: Stub _getsockname
        // ==========================================================
        if symbol == "_getsockname" {
            fn fake_getsockname(
                env: &mut crate::Environment,
                _socket: i32,
                addr: crate::mem::MutPtr<crate::libc::sys::socket::sockaddr>,
                addrlen: crate::mem::MutPtr<crate::libc::netdb::socklen_t>,
            ) -> i32 {
                log!("_getsockname called (RR2 bypass)");
                // Return a dummy address (0.0.0.0:0)
                let dummy = crate::libc::sys::socket::sockaddr::from_ipv4_parts([0, 0, 0, 0], 0);
                env.mem.write(addr, dummy);
                env.mem.write(addrlen, 16); // sizeof(sockaddr_in)
                0
            }
            return Some(
                &(fake_getsockname
                    as fn(
                        &mut crate::Environment,
                        i32,
                        crate::mem::MutPtr<crate::libc::sys::socket::sockaddr>,
                        crate::mem::MutPtr<crate::libc::netdb::socklen_t>,
                    ) -> i32),
            );
        }
        // ==========================================================
        // 🏎️ REAL RACING 2 BYPASS: Stub _NSGetExecutablePath
        // ==========================================================
        if symbol == "__NSGetExecutablePath" {
            fn fake_NSGetExecutablePath(
                env: &mut crate::Environment,
                buf: crate::mem::MutPtr<u8>,
                bufsize: crate::mem::MutPtr<u32>,
            ) -> i32 {
                log!("_NSGetExecutablePath called (RR2 bypass)");
                let path = b"/path/to/app.app/app\0";
                let required_size = path.len() as u32;

                let current_size = env.mem.read(bufsize);
                if current_size < required_size {
                    env.mem.write(bufsize, required_size);
                    return -1;
                }

                for (i, &b) in path.iter().enumerate() {
                    env.mem.write(buf + i as u32, b);
                }
                0
            }
            return Some(
                &(fake_NSGetExecutablePath
                    as fn(&mut crate::Environment, crate::mem::MutPtr<u8>, crate::mem::MutPtr<u32>) -> i32),
            );
        }

        // ==========================================================
        // 🏎️ GAMELOFT BYPASS: Safely intercept _pthread_exit
        // ==========================================================
        if symbol == "_pthread_exit" {
            fn fake_pthread_exit(env: &mut crate::Environment, ret_val: u32) {
                println!("🎮 LOG: Safely intercepted pthread_exit({:#x}) to prevent panic!", ret_val);
                
                // 1. Ensure the thread's return value is placed in CPU register 0
                env.cpu.regs_mut()[0] = ret_val; 
                
                // 2. Safely branch to touchHLE's built-in thread cleanup routine
                let exit_routine = env.dyld.thread_exit_routine();
                env.cpu.branch(exit_routine); 
            }
            return Some(&(fake_pthread_exit as fn(&mut crate::Environment, u32) -> ()));
        }
        // ==========================================================
// 🏎️ BYPASS: _pthread_setname_np (thread naming, safe to ignore)
// ==========================================================
if symbol == "_pthread_setname_np" {
    fn fake_pthread_setname_np(_env: &mut crate::Environment, _thread: u32, _name: crate::mem::ConstPtr<u8>) -> i32 {
        log!("_pthread_setname_np bypassed");
        0 // success
    }
    return Some(
        &(fake_pthread_setname_np
            as fn(&mut crate::Environment, u32, crate::mem::ConstPtr<u8>) -> i32),
    );
}

        // ==========================================================
        // 🏎️ EA BYPASS: Safely absorb IPSP ADK Logger
        // ==========================================================
        if symbol == "_ipsp_logger" { // Replace with the exact symbol from your log!
            fn fake_ipsp_logger(env: &mut crate::Environment) {
                println!("🎮 LOG: Safely absorbed IPSP ADK logger call!");
                env.cpu.regs_mut()[0] = 0; // Return success
            }
            return Some(&(fake_ipsp_logger as fn(&mut crate::Environment) -> ()));
        }
        
        // ImplDifftime
        if symbol == "_difftime" {
            fn impl_difftime(_env: &mut crate::Environment, time1: i32, time0: i32) -> f64 {
                (time1 as f64) - (time0 as f64)
            }
            return Some(&(impl_difftime as fn(&mut crate::Environment, i32, i32) -> f64));
        }

        // ImplStrncatChk
        if symbol == "___strncat_chk" {
            fn impl_strncat_chk(
                env: &mut crate::Environment,
                dest: crate::mem::MutPtr<u8>,
                src: crate::mem::ConstPtr<u8>,
                n: u32,
                destlen: u32,
            ) -> crate::mem::MutPtr<u8> {
                let src_str = env.mem.cstr_at_utf8(src).unwrap_or("<invalid utf8>");
                log_dbg!(
                    "___strncat_chk(dest: {:?}, src: {:?} ('{}'), n: {}, destlen: {})",
                    dest,
                    src,
                    src_str,
                    n,
                    destlen
                );

                let mut dest_len = 0;
                while env.mem.read(dest + dest_len) != 0 {
                    dest_len += 1;
                }

                let mut i = 0;
                while i < n {
                    let c = env.mem.read(src + i);
                    if c == 0 {
                        break;
                    }
                    env.mem.write(dest + dest_len + i, c);
                    i += 1;
                }
                env.mem.write(dest + dest_len + i, 0);

                dest
            }
            return Some(
                &(impl_strncat_chk
                    as fn(
                        &mut crate::Environment,
                        crate::mem::MutPtr<u8>,
                        crate::mem::ConstPtr<u8>,
                        u32,
                        u32,
                    ) -> crate::mem::MutPtr<u8>),
            );
        }

        if symbol == "__dyld_register_func_for_add_image" {
            fn fake_dyld_register(_env: &mut crate::Environment, _func: u32) {
                println!("🎮 LOG: Bypassing __dyld_register_func_for_add_image (Crashlytics/Analytics disabled)!");
            }
            return Some(&(fake_dyld_register as fn(&mut crate::Environment, u32) -> ()));
        }
        if symbol == "__dyld_register_func_for_remove_image" {
           fn fake_dyld_register_remove(_env: &mut crate::Environment, _func: u32) {
               println!("🎮 LOG: Bypassing __dyld_register_func_for_remove_image (Crashlytics/Analytics disabled)!");
                  }
         return Some(&(fake_dyld_register_remove as fn(&mut crate::Environment, u32) -> ()));
        }

        if symbol == "__Znwm" || symbol == "__Znwj" {
            fn fake_cpp_new(env: &mut crate::Environment, size: u32) -> u32 {
                println!("🎮 LOG: Intercepted C++ new({}), allocating guest memory!", size);
                env.mem.alloc(size).to_bits()
            }
            return Some(&(fake_cpp_new as fn(&mut crate::Environment, u32) -> u32));
        }

        if symbol == "__ZdlPv" || symbol == "__ZdaPv" {
            fn fake_cpp_delete(_env: &mut crate::Environment, _ptr: u32) {
                // Ignore delete to prevent crashes on unmapped memory
            }
            return Some(&(fake_cpp_delete as fn(&mut crate::Environment, u32) -> ()));
        }
        
        if symbol == "_getipnodebyname" {
            fn fake_getipnodebyname(
                env: &mut crate::Environment,
                name_ptr: u32,
                _af: i32,
                _flags: i32,
                error_num_ptr: u32,
            ) -> u32 {
                let name = if name_ptr != 0 {
                    env.mem.cstr_at_utf8(crate::mem::ConstPtr::<u8>::from_bits(name_ptr)).unwrap_or("unknown")
                } else {
                    "null"
                };
                println!("🎮 LOG: DNS Sinkholed getipnodebyname request to: {}", name);
                
                // Set the error number pointer to HOST_NOT_FOUND (1) so the game knows it failed safely
                if error_num_ptr != 0 {
                    env.mem.write(crate::mem::MutPtr::<i32>::from_bits(error_num_ptr), 1);
                }
                
                0 // Return NULL to instantly fail the connection!
            }
            return Some(
                &(fake_getipnodebyname
                    as fn(&mut crate::Environment, u32, i32, i32, u32) -> u32),
            );
        }
if symbol == "_CTFontCopyGraphicsFont" {
    fn ct_copy_graphics_font(env: &mut Environment, _ct_font: u32, _attributes: u32) -> u32 {
        log_dbg!("CTFontCopyGraphicsFont stub called");
        use crate::objc::TrivialHostObject;
        let class = env.objc.get_known_class("_touchHLE_CGFont", &mut env.mem);
        let dummy_font = env.objc.alloc_object(class, Box::new(TrivialHostObject), &mut env.mem);
        dummy_font.to_bits()
    }
    return Some(&(ct_copy_graphics_font as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CTFontCreateWithName" {
    fn ct_create_with_name(env: &mut Environment, _name: u32, _size: f32, _matrix: u32) -> u32 {
        log_dbg!("CTFontCreateWithName stub called");
        use crate::objc::{msg, msg_class};
        use crate::frameworks::foundation::ns_string::from_rust_string;
        let uifont_class = msg_class![env; UIFont class];
        let font: u32 = msg![env; uifont_class systemFontOfSize:17.0];
        font
    }
    return Some(&(ct_create_with_name as fn(&mut Environment, u32, f32, u32) -> u32));
}
if symbol == "_CTFontCreateWithFontDescriptor" {
    fn ct_create_with_descriptor(env: &mut Environment, _desc: u32, _size: f32, _matrix: u32) -> u32 {
        log_dbg!("CTFontCreateWithFontDescriptor stub called");
        use crate::objc::{msg, msg_class};
        let uifont_class = msg_class![env; UIFont class];
        let font: u32 = msg![env; uifont_class systemFontOfSize:17.0];
        font
    }
    return Some(&(ct_create_with_descriptor as fn(&mut Environment, u32, f32, u32) -> u32));
}
if symbol == "_CTFontGetGlyphsForCharacters" {
    fn ct_get_glyphs(_env: &mut Environment, _font: u32, _chars: u32, _glyphs: u32, _count: u32) -> bool {
        log_dbg!("CTFontGetGlyphsForCharacters stub called -> true");
        true
    }
    return Some(&(ct_get_glyphs as fn(&mut Environment, u32, u32, u32, u32) -> bool));
}
if symbol == "_CTFontGetAdvancesForGlyphs" {
    fn ct_get_advances(_env: &mut Environment, _font: u32, _orientation: u32, _glyphs: u32, _advances: u32, _count: u32) -> f64 {
        log_dbg!("CTFontGetAdvancesForGlyphs stub called -> 0.0");
        0.0
    }
    return Some(&(ct_get_advances as fn(&mut Environment, u32, u32, u32, u32, u32) -> f64));
}
if symbol == "_CTFontGetBoundingBoxesForGlyphs" {
    fn ct_get_bboxes(_env: &mut Environment, _font: u32, _orientation: u32, _glyphs: u32, _bboxes: u32, _count: u32) -> f64 {
        log_dbg!("CTFontGetBoundingBoxesForGlyphs stub called -> 0.0");
        0.0
    }
    return Some(&(ct_get_bboxes as fn(&mut Environment, u32, u32, u32, u32, u32) -> f64));
}
if symbol == "_CTFontGetAscent" {
    fn ct_get_ascent(_env: &mut Environment, _font: u32) -> f64 { 0.8 }
    return Some(&(ct_get_ascent as fn(&mut Environment, u32) -> f64));
}
if symbol == "_CTFontGetDescent" {
    fn ct_get_descent(_env: &mut Environment, _font: u32) -> f64 { 0.2 }
    return Some(&(ct_get_descent as fn(&mut Environment, u32) -> f64));
}
if symbol == "_CTFontGetLeading" {
    fn ct_get_leading(_env: &mut Environment, _font: u32) -> f64 { 0.1 }
    return Some(&(ct_get_leading as fn(&mut Environment, u32) -> f64));
}
if symbol == "_CTFontGetUnitsPerEm" {
    fn ct_get_units(_env: &mut Environment, _font: u32) -> u32 { 1000 }
    return Some(&(ct_get_units as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontGetCapHeight" {
    fn ct_get_cap(_env: &mut Environment, _font: u32) -> f64 { 0.6 }
    return Some(&(ct_get_cap as fn(&mut Environment, u32) -> f64));
}
if symbol == "_CTFontGetXHeight" {
    fn ct_get_x(_env: &mut Environment, _font: u32) -> f64 { 0.4 }
    return Some(&(ct_get_x as fn(&mut Environment, u32) -> f64));
}
if symbol == "_CTFontGetGlyphCount" {
    fn ct_get_glyph_count(_env: &mut Environment, _font: u32) -> u32 { 256 }
    return Some(&(ct_get_glyph_count as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontGetBoundingBox" {
    fn ct_get_bbox(_env: &mut Environment, _font: u32) -> u32 { 0 }
    return Some(&(ct_get_bbox as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontGetUnderlinePosition" {
    fn ct_get_ul_pos(_env: &mut Environment, _font: u32) -> f64 { 0.0 }
    return Some(&(ct_get_ul_pos as fn(&mut Environment, u32) -> f64));
}
if symbol == "_CTFontGetUnderlineThickness" {
    fn ct_get_ul_thick(_env: &mut Environment, _font: u32) -> f64 { 0.05 }
    return Some(&(ct_get_ul_thick as fn(&mut Environment, u32) -> f64));
}
if symbol == "_CTFontCreateWithGraphicsFont" {
    fn ct_font_stub(env: &mut Environment, _cg_font: u32, size: f32, _transform: u32, _attributes: u32) -> u32 {
        use crate::objc::{msg, msg_class, nil};
        use crate::frameworks::foundation::ns_string::from_rust_string;

        log_dbg!("CTFontCreateWithGraphicsFont stub called, size={}", size);
        let default_size = if size == 0.0 { 17.0 } else { size };
        let font_name = "Helvetica";
        let name_ns = from_rust_string(env, font_name.to_string());
        let uifont_class = msg_class![env; UIFont class];
        let font: u32 = msg![env; uifont_class fontWithName:name_ns size:default_size];
        font as u32
    }
    return Some(&(ct_font_stub as fn(&mut Environment, u32, f32, u32, u32) -> u32));
}
if symbol == "___snprintf_chk" {
    fn snprintf_chk_stub(_env: &mut Environment, _str: u32, _size: u32, _flags: u32, _fmt: u32) -> i32 {
        log_dbg!("___snprintf_chk stub called, ignoring");
        0
    }
    return Some(&(snprintf_chk_stub as fn(&mut Environment, u32, u32, u32, u32) -> i32));
}
if symbol == "_CTFontDescriptorCreateWithAttributes" {
    fn ct_descriptor_create_with_attributes(env: &mut Environment, _attributes: u32) -> u32 {
        log_dbg!("CTFontDescriptorCreateWithAttributes stub called");
        use crate::objc::{msg_class, TrivialHostObject};
        let obj_class = msg_class![env; NSObject class];
        let dummy = env.objc.alloc_object(obj_class, Box::new(TrivialHostObject), &mut env.mem);
        dummy.to_bits()
    }
    return Some(&(ct_descriptor_create_with_attributes as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontDescriptorCreateWithNameAndSize" {
    fn ct_descriptor_create_with_name_and_size(_env: &mut Environment, _name: u32, _size: f32, _matrix: u32) -> u32 {
        log_dbg!("CTFontDescriptorCreateWithNameAndSize stub called");
        0
    }
    return Some(&(ct_descriptor_create_with_name_and_size as fn(&mut Environment, u32, f32, u32) -> u32));
}
if symbol == "_CTFontCopyFamilyName" {
    fn ct_copy_family_name(env: &mut Environment, _font: u32) -> u32 {
        log_dbg!("CTFontCopyFamilyName stub called");
        use crate::frameworks::foundation::ns_string::from_rust_string;
        let name = from_rust_string(env, "Helvetica".to_string());
        name.to_bits()
    }
    return Some(&(ct_copy_family_name as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontCopyDisplayName" {
    fn ct_copy_display_name(env: &mut Environment, _font: u32) -> u32 {
        log_dbg!("CTFontCopyDisplayName stub called");
        use crate::frameworks::foundation::ns_string::from_rust_string;
        let name = from_rust_string(env, "Helvetica".to_string());
        name.to_bits()
    }
    return Some(&(ct_copy_display_name as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontCopyPostScriptName" {
    fn ct_copy_postscript_name(env: &mut Environment, _font: u32) -> u32 {
        log_dbg!("CTFontCopyPostScriptName stub called");
        use crate::frameworks::foundation::ns_string::from_rust_string;
        let name = from_rust_string(env, "Helvetica".to_string());
        name.to_bits()
    }
    return Some(&(ct_copy_postscript_name as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontGetSize" {
    fn ct_get_size(_env: &mut Environment, _font: u32) -> f64 {
        log_dbg!("CTFontGetSize stub called -> 17.0");
        17.0
    }
    return Some(&(ct_get_size as fn(&mut Environment, u32) -> f64));
}
if symbol == "_CTFontGetMatrix" {
    fn ct_get_matrix(_env: &mut Environment, _font: u32) -> u32 {
        log_dbg!("CTFontGetMatrix stub called");
        0 // identity matrix (CGAffineTransformIdentity)
    }
    return Some(&(ct_get_matrix as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontGetSymbolicTraits" {
    fn ct_get_traits(_env: &mut Environment, _font: u32) -> u32 {
        log_dbg!("CTFontGetSymbolicTraits stub called -> 0");
        0
    }
    return Some(&(ct_get_traits as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontCopyAttribute" {
    fn ct_copy_attribute(env: &mut Environment, _font: u32, _attr: u32) -> u32 {
        log_dbg!("CTFontCopyAttribute stub called");
        0 // nil
    }
    return Some(&(ct_copy_attribute as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CTFontCopyAvailableTables" {
    fn ct_copy_available_tables(_env: &mut Environment, _font: u32, _options: u32) -> u32 {
        log_dbg!("CTFontCopyAvailableTables stub called");
        0 // nil (no tables)
    }
    return Some(&(ct_copy_available_tables as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CTFontGetGlyphWithName" {
    fn ct_get_glyph_with_name(_env: &mut Environment, _font: u32, _glyph_name: u32) -> u16 {
        log_dbg!("CTFontGetGlyphWithName stub called -> 0");
        0
    }
    return Some(&(ct_get_glyph_with_name as fn(&mut Environment, u32, u32) -> u16));
}
if symbol == "_CTTypesetterCreateWithAttributedString" {
    fn ct_typesetter_create(env: &mut Environment, _attr_str: u32) -> u32 {
        log_dbg!("CTTypesetterCreateWithAttributedString stub called");
        use crate::objc::{msg_class, TrivialHostObject};
        let obj_class = msg_class![env; NSObject class];
        let dummy = env.objc.alloc_object(obj_class, Box::new(TrivialHostObject), &mut env.mem);
        dummy.to_bits()
    }
    return Some(&(ct_typesetter_create as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTTypesetterSuggestLineBreak" {
    fn ct_typesetter_suggest_line_break(_env: &mut Environment, _ts: u32, _start_index: u32, _width: f64) -> u32 { 0 }
    return Some(&(ct_typesetter_suggest_line_break as fn(&mut Environment, u32, u32, f64) -> u32));
}
if symbol == "_CTTypesetterSuggestClusterBreak" {
    fn ct_typesetter_suggest_cluster_break(_env: &mut Environment, _ts: u32, _start_index: u32, _width: f64) -> u32 { 0 }
    return Some(&(ct_typesetter_suggest_cluster_break as fn(&mut Environment, u32, u32, f64) -> u32));
}
if symbol == "_CTLineCreateWithAttributedString" {
    fn ct_line_create_with_attr_string(env: &mut Environment, _attr_str: u32) -> u32 {
        log_dbg!("CTLineCreateWithAttributedString stub called");
        use crate::objc::{msg_class, TrivialHostObject};
        let obj_class = msg_class![env; NSObject class];
        let dummy = env.objc.alloc_object(obj_class, Box::new(TrivialHostObject), &mut env.mem);
        dummy.to_bits()
    }
    return Some(&(ct_line_create_with_attr_string as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTLineGetGlyphRuns" {
    fn ct_line_get_glyph_runs(_env: &mut Environment, _line: u32) -> u32 { 0 }
    return Some(&(ct_line_get_glyph_runs as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTLineGetTypographicBounds" {
    fn ct_line_get_typographic_bounds(_env: &mut Environment, _line: u32, _ascent: u32, _descent: u32, _leading: u32) -> f64 { 0.0 }
    return Some(&(ct_line_get_typographic_bounds as fn(&mut Environment, u32, u32, u32, u32) -> f64));
}
if symbol == "_CTRunGetGlyphs" {
    fn ct_run_get_glyphs(_env: &mut Environment, _run: u32, _range: u32, _glyphs: u32) { }
    return Some(&(ct_run_get_glyphs as fn(&mut Environment, u32, u32, u32) -> ()));
}
if symbol == "_CTRunGetAttributes" {
    fn ct_run_get_attributes(_env: &mut Environment, _run: u32) -> u32 { 0 }
    return Some(&(ct_run_get_attributes as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontCopyName" {
    fn ct_copy_name(env: &mut Environment, _font: u32, _name_key: u32) -> u32 {
        log_dbg!("CTFontCopyName stub called");
        use crate::frameworks::foundation::ns_string::from_rust_string;
        let dummy = from_rust_string(env, "Helvetica".to_string());
        dummy.to_bits()
    }
    return Some(&(ct_copy_name as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CTFontCopyLocalizedName" {
    fn ct_copy_localized_name(env: &mut Environment, _font: u32, _name_key: u32) -> u32 {
        log_dbg!("CTFontCopyLocalizedName stub called");
        use crate::frameworks::foundation::ns_string::from_rust_string;
        let dummy = from_rust_string(env, "Helvetica".to_string());
        dummy.to_bits()
    }
    return Some(&(ct_copy_localized_name as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CTFrameGetLines" {
    fn ct_frame_get_lines(_env: &mut Environment, _frame: u32, _range: u32, _origins: u32) -> u32 { 0 }
    return Some(&(ct_frame_get_lines as fn(&mut Environment, u32, u32, u32) -> u32));
}
if symbol == "_CTFrameGetLineOrigins" {
    fn ct_frame_get_line_origins(_env: &mut Environment, _frame: u32, _range: u32, _origins: u32) { }
    return Some(&(ct_frame_get_line_origins as fn(&mut Environment, u32, u32, u32) -> ()));
}
if symbol == "_CTFramesetterCreateWithAttributedString" {
    fn ct_framesetter_create(env: &mut Environment, _attr_str: u32) -> u32 {
        log_dbg!("CTFramesetterCreateWithAttributedString stub called");
        use crate::objc::{msg_class, TrivialHostObject};
        let obj_class = msg_class![env; NSObject class];
        let dummy = env.objc.alloc_object(obj_class, Box::new(TrivialHostObject), &mut env.mem);
        dummy.to_bits()
    }
    return Some(&(ct_framesetter_create as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFramesetterCreateFrame" {
    fn ct_framesetter_create_frame(env: &mut Environment, _fs: u32, _string_range: u32, _path: u32, _frame_attrs: u32) -> u32 {
        log_dbg!("CTFramesetterCreateFrame stub called");
        use crate::objc::{msg_class, TrivialHostObject};
        let obj_class = msg_class![env; NSObject class];
        let dummy = env.objc.alloc_object(obj_class, Box::new(TrivialHostObject), &mut env.mem);
        dummy.to_bits()
    }
    return Some(&(ct_framesetter_create_frame as fn(&mut Environment, u32, u32, u32, u32) -> u32));
}
if symbol == "_CTFramesetterSuggestFrameSizeWithConstraints" {
    fn ct_framesetter_suggest_size(_env: &mut Environment, _fs: u32, _string_range: u32, _frame_attrs: u32, _constraints: u32, _fit_range: u32) -> f64 { 0.0 }
    return Some(&(ct_framesetter_suggest_size as fn(&mut Environment, u32, u32, u32, u32, u32) -> f64));
}
if symbol == "_CTParagraphStyleCreate" {
    fn ct_paragraph_style_create(_env: &mut Environment, _settings: u32, _count: u32) -> u32 { 0 }
    return Some(&(ct_paragraph_style_create as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CTParagraphStyleGetValue" {
    fn ct_paragraph_style_get_value(_env: &mut Environment, _style: u32, _spec: u32, _buf: u32) -> bool { false }
    return Some(&(ct_paragraph_style_get_value as fn(&mut Environment, u32, u32, u32) -> bool));
}
if symbol == "_CTLineGetStringRange" {
    fn ct_line_get_string_range(_env: &mut Environment, _line: u32) -> u64 { 0 }
    return Some(&(ct_line_get_string_range as fn(&mut Environment, u32) -> u64));
}
if symbol == "_CTRunGetStringRange" {
    fn ct_run_get_string_range(_env: &mut Environment, _run: u32) -> u64 { 0 }
    return Some(&(ct_run_get_string_range as fn(&mut Environment, u32) -> u64));
}
if symbol == "_CTRunGetStatus" {
    fn ct_run_get_status(_env: &mut Environment, _run: u32) -> u32 { 0 }
    return Some(&(ct_run_get_status as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontManagerCreateFontDescriptorFromData" {
    fn ct_font_manager_create_descriptor(env: &mut Environment, _data: u32) -> u32 {
        log_dbg!("CTFontManagerCreateFontDescriptorFromData stub called");
        use crate::objc::{msg_class, TrivialHostObject};
        let obj_class = msg_class![env; NSObject class];
        let dummy = env.objc.alloc_object(obj_class, Box::new(TrivialHostObject), &mut env.mem);
        dummy.to_bits()
    }
    return Some(&(ct_font_manager_create_descriptor as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTFontManagerRegisterGraphicsFont" {
    fn ct_font_manager_register_font(_env: &mut Environment, _font: u32, _error: u32) -> bool { true }
    return Some(&(ct_font_manager_register_font as fn(&mut Environment, u32, u32) -> bool));
}
if symbol == "_CTTypesetterCreateLine" {
    fn ct_typesetter_create_line(env: &mut Environment, _ts: u32, _range: u32) -> u32 {
        log_dbg!("CTTypesetterCreateLine stub called");
        use crate::objc::{msg_class, TrivialHostObject};
        let obj_class = msg_class![env; NSObject class];
        let dummy = env.objc.alloc_object(obj_class, Box::new(TrivialHostObject), &mut env.mem);
        dummy.to_bits()
    }
    return Some(&(ct_typesetter_create_line as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CTLineGetGlyphCount" {
    fn ct_line_get_glyph_count(_env: &mut Environment, _line: u32) -> u32 {
        log_dbg!("CTLineGetGlyphCount stub called -> 0");
        0
    }
    return Some(&(ct_line_get_glyph_count as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTRunGetGlyphCount" {
    fn ct_run_get_glyph_count(_env: &mut Environment, _run: u32) -> u32 {
        log_dbg!("CTRunGetGlyphCount stub called -> 0");
        0
    }
    return Some(&(ct_run_get_glyph_count as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CTRunGetPositions" {
    fn ct_run_get_positions(_env: &mut Environment, _run: u32, _range: u32, _positions: u32) {
        log_dbg!("CTRunGetPositions stub called (ignored)");
    }
    return Some(&(ct_run_get_positions as fn(&mut Environment, u32, u32, u32) -> ()));
}
if symbol == "_CTFontDescriptorCreateMatchingFontDescriptors" {
    fn ct_descriptor_create_matching(env: &mut Environment, _descriptor: u32, _mandatory_attrs: u32) -> u32 {
        log_dbg!("CTFontDescriptorCreateMatchingFontDescriptors stub called");
        // Return an empty array (CFArrayRef) – dummy NSArray
        use crate::objc::{id, msg_class};
        let arr: id = msg_class![env; NSArray array];
        arr.to_bits()
    }
    return Some(&(ct_descriptor_create_matching as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CTFrameDraw" {
    fn ct_frame_draw(_env: &mut Environment, _frame: u32, _context: u32) {
        log_dbg!("CTFrameDraw stub called (ignored)");
    }
    return Some(&(ct_frame_draw as fn(&mut Environment, u32, u32) -> ()));
}
if symbol == "_CTLineDraw" {
    fn ct_line_draw(_env: &mut Environment, _line: u32, _context: u32) {
        log_dbg!("CTLineDraw stub called (ignored)");
    }
    return Some(&(ct_line_draw as fn(&mut Environment, u32, u32) -> ()));
}
if symbol == "_CTLineGetImageBounds" {
    fn ct_line_get_image_bounds(_env: &mut Environment, _line: u32, _context: u32) -> u64 {
        log_dbg!("CTLineGetImageBounds stub called -> CGRectZero");
        0
    }
    return Some(&(ct_line_get_image_bounds as fn(&mut Environment, u32, u32) -> u64));
}
if symbol == "_CFArrayGetCount" {
    fn cf_array_get_count(_env: &mut Environment, _array: u32) -> u32 { 0 }
    return Some(&(cf_array_get_count as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CFArrayGetValueAtIndex" {
    fn cf_array_get_value(_env: &mut Environment, _array: u32, _idx: u32) -> u32 { 0 }
    return Some(&(cf_array_get_value as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CFDataCreate" {
    fn cf_data_create(_env: &mut Environment, _alloc: u32, _bytes: u32, _len: u32) -> u32 { 0 }
    return Some(&(cf_data_create as fn(&mut Environment, u32, u32, u32) -> u32));
}
if symbol == "_CFDataGetBytePtr" {
    fn cf_data_get_byte_ptr(_env: &mut Environment, _data: u32) -> u32 { 0 }
    return Some(&(cf_data_get_byte_ptr as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CFDataGetLength" {
    fn cf_data_get_length(_env: &mut Environment, _data: u32) -> u32 { 0 }
    return Some(&(cf_data_get_length as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CFDictionaryGetValue" {
    fn cf_dict_get_value(_env: &mut Environment, _dict: u32, _key: u32) -> u32 { 0 }
    return Some(&(cf_dict_get_value as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CFNumberCreate" {
    fn cf_number_create(_env: &mut Environment, _alloc: u32, _type: u32, _value_ptr: u32) -> u32 { 0 }
    return Some(&(cf_number_create as fn(&mut Environment, u32, u32, u32) -> u32));
}
if symbol == "_CFNumberGetValue" {
    fn cf_number_get_value(_env: &mut Environment, _num: u32, _type: u32, _out: u32) -> bool { false }
    return Some(&(cf_number_get_value as fn(&mut Environment, u32, u32, u32) -> bool));
}
if symbol == "_CFReadStreamOpen" {
    fn cf_read_stream_open(_env: &mut Environment, _stream: u32) -> bool { true }
    return Some(&(cf_read_stream_open as fn(&mut Environment, u32) -> bool));
}
if symbol == "_CFReadStreamRead" {
    fn cf_read_stream_read(_env: &mut Environment, _stream: u32, _buf: u32, _len: u32) -> u32 { 0 }
    return Some(&(cf_read_stream_read as fn(&mut Environment, u32, u32, u32) -> u32));
}
if symbol == "_CFReadStreamClose" {
    fn cf_read_stream_close(_env: &mut Environment, _stream: u32) { }
    return Some(&(cf_read_stream_close as fn(&mut Environment, u32) -> ()));
}
if symbol == "_CFStringCreateWithBytes" {
    fn fake_CFStringCreateWithBytes(
        env: &mut crate::Environment,
        _alloc: u32,
        bytes: crate::mem::ConstPtr<u8>,
        numBytes: u32,
        _encoding: u32,
        _isExternal: bool,
    ) -> u32 {
        use crate::frameworks::foundation::ns_string::from_rust_string;
        let slice = env.mem.bytes_at(bytes, numBytes);
        // Convert bytes to a String (lossy UTF-8)
        let s = String::from_utf8_lossy(slice).to_string();
        let nsstr = from_rust_string(env, s);
        nsstr.to_bits()
    }
    return Some(&(fake_CFStringCreateWithBytes as fn(&mut crate::Environment, u32, crate::mem::ConstPtr<u8>, u32, u32, bool) -> u32));
}
if symbol == "_CFStringGetCString" {
    fn cf_string_get_cstring(_env: &mut Environment, _str: u32, _buf: u32, _buf_size: u32, _encoding: u32) -> bool { false }
    return Some(&(cf_string_get_cstring as fn(&mut Environment, u32, u32, u32, u32) -> bool));
}
if symbol == "_CFURLResourceIsReachable" {
    fn cf_url_resource_is_reachable(_env: &mut Environment, _url: u32, _error: u32) -> bool { false }
    return Some(&(cf_url_resource_is_reachable as fn(&mut Environment, u32, u32) -> bool));
}
if symbol == "_CFAttributedStringCreate" {
    fn cf_attributed_string_create(_env: &mut Environment, _alloc: u32, _str: u32, _attrs: u32) -> u32 { 0 }
    return Some(&(cf_attributed_string_create as fn(&mut Environment, u32, u32, u32) -> u32));
}
if symbol == "_CFBooleanGetValue" {
    fn cf_boolean_get_value(_env: &mut Environment, _bool: u32) -> bool { false }
    return Some(&(cf_boolean_get_value as fn(&mut Environment, u32) -> bool));
}
if symbol == "_CFBundleCopyBundleURL" {
    fn cf_bundle_copy_bundle_url(_env: &mut Environment, _bundle: u32) -> u32 { 0 }
    return Some(&(cf_bundle_copy_bundle_url as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CFBundleCopyResourceURL" {
    fn cf_bundle_copy_resource_url(_env: &mut Environment, _bundle: u32, _res_name: u32, _res_type: u32, _sub_dir: u32) -> u32 { 0 }
    return Some(&(cf_bundle_copy_resource_url as fn(&mut Environment, u32, u32, u32, u32) -> u32));
}
if symbol == "_CFBundleGetBundleWithIdentifier" {
    fn cf_bundle_get_with_id(_env: &mut Environment, _id: u32) -> u32 { 0 }
    return Some(&(cf_bundle_get_with_id as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CFBundleGetIdentifier" {
    fn cf_bundle_get_id(_env: &mut Environment, _bundle: u32) -> u32 { 0 }
    return Some(&(cf_bundle_get_id as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CFBundleGetMainBundle" {
    fn cf_bundle_get_main(_env: &mut Environment) -> u32 { 0 }
    return Some(&(cf_bundle_get_main as fn(&mut Environment) -> u32));
}
if symbol == "_CFBundleGetValueForInfoDictionaryKey" {
    fn cf_bundle_get_value(_env: &mut Environment, _bundle: u32, _key: u32) -> u32 { 0 }
    return Some(&(cf_bundle_get_value as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CFCharacterSetCreateWithCharactersInRange" {
    fn cf_charset_create_range(_env: &mut Environment, _alloc: u32, _range: u32) -> u32 { 0 }
    return Some(&(cf_charset_create_range as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CFCharacterSetGetLongCharacterBitmap" {
    fn cf_charset_get_bitmap(_env: &mut Environment, _cs: u32) -> u32 { 0 }
    return Some(&(cf_charset_get_bitmap as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CFDataCreateMutable" {
    fn cf_data_create_mutable(_env: &mut Environment, _alloc: u32, _capacity: u32) -> u32 { 0 }
    return Some(&(cf_data_create_mutable as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CFDictionaryCreateMutable" {
    fn cf_dict_create_mutable(_env: &mut Environment, _alloc: u32, _capacity: u32, _key_callbacks: u32, _value_callbacks: u32) -> u32 { 0 }
    return Some(&(cf_dict_create_mutable as fn(&mut Environment, u32, u32, u32, u32) -> u32));
}
if symbol == "_CFDictionarySetValue" {
    fn cf_dict_set_value(_env: &mut Environment, _dict: u32, _key: u32, _value: u32) { }
    return Some(&(cf_dict_set_value as fn(&mut Environment, u32, u32, u32) -> ()));
}
if symbol == "_CFLocaleCopyCurrent" {
    fn cf_locale_copy_current(_env: &mut Environment) -> u32 { 0 }
    return Some(&(cf_locale_copy_current as fn(&mut Environment) -> u32));
}
if symbol == "_CFLocaleGetIdentifier" {
    fn cf_locale_get_id(_env: &mut Environment, _locale: u32) -> u32 { 0 }
    return Some(&(cf_locale_get_id as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CFStringCreateWithFormat" {
    fn cf_string_create_format(_env: &mut Environment, _alloc: u32, _format_options: u32, _format: u32) -> u32 {
        // Varargs are ignored; return dummy string object
        use crate::frameworks::foundation::ns_string::get_static_str;
        let dummy = get_static_str(_env, "");
        dummy.to_bits()
    }
    return Some(&(cf_string_create_format as fn(&mut Environment, u32, u32, u32) -> u32));
}
if symbol == "_CFStringGetLength" {
    fn cf_string_get_length(_env: &mut Environment, _str: u32) -> u32 { 0 }
    return Some(&(cf_string_get_length as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CFStringGetCharacterAtIndex" {
    fn cf_string_get_char(_env: &mut Environment, _str: u32, _idx: u32) -> u16 { 0 }
    return Some(&(cf_string_get_char as fn(&mut Environment, u32, u32) -> u16));
}
if symbol == "_CFURLCreateWithString" {
    fn cf_url_create_with_string(_env: &mut Environment, _alloc: u32, _url_str: u32, _base_url: u32) -> u32 { 0 }
    return Some(&(cf_url_create_with_string as fn(&mut Environment, u32, u32, u32) -> u32));
}
if symbol == "_CFURLGetString" {
    fn cf_url_get_string(_env: &mut Environment, _url: u32) -> u32 { 0 }
    return Some(&(cf_url_get_string as fn(&mut Environment, u32) -> u32));
}
if symbol == "_CFUUIDCreateFromString" {
    fn cf_uuid_create_from_string(_env: &mut Environment, _alloc: u32, _str: u32) -> u32 { 0 }
    return Some(&(cf_uuid_create_from_string as fn(&mut Environment, u32, u32) -> u32));
}
if symbol == "_CFUUIDGetUUIDBytes" {
    fn cf_uuid_get_bytes(_env: &mut Environment, _uuid: u32) -> u64 { 0 }
    return Some(&(cf_uuid_get_bytes as fn(&mut Environment, u32) -> u64));
}
if symbol == "_CFStringCreateWithBytes" {
    fn fake_CFStringCreateWithBytes(
        env: &mut crate::Environment,
        _alloc: u32,
        bytes: crate::mem::ConstPtr<u8>,
        numBytes: u32,
        _encoding: u32,
        _isExternal: bool,
    ) -> u32 {
        use crate::frameworks::foundation::ns_string::from_rust_bytes;
        let slice = env.mem.bytes_at(bytes, numBytes);
        let str = String::from_utf8_lossy(slice).to_string();
        let nsstr = from_rust_bytes(env, str.as_bytes());
        nsstr.to_bits()
    }
    return Some(&(fake_CFStringCreateWithBytes as fn(&mut crate::Environment, u32, crate::mem::ConstPtr<u8>, u32, u32, bool) -> u32));
}

if symbol == "_CFStringCreateWithCString" {
    fn fake_CFStringCreateWithCString(
        env: &mut crate::Environment,
        _alloc: u32,
        cStr: crate::mem::ConstPtr<u8>,
        _encoding: u32,
    ) -> u32 {
        use crate::frameworks::foundation::ns_string::from_rust_string;
        let rust_str = env.mem.cstr_at_utf8(cStr).unwrap_or_default();
        let nsstr = from_rust_string(env, rust_str.to_string());
        nsstr.to_bits()
    }
    return Some(&(fake_CFStringCreateWithCString as fn(&mut crate::Environment, u32, crate::mem::ConstPtr<u8>, u32) -> u32));
}

if symbol == "_CFStringCreateWithFormat" {
    fn fake_CFStringCreateWithFormat(
        env: &mut crate::Environment,
        _alloc: u32,
        _formatOptions: u32,
        format: crate::mem::ConstPtr<u8>,
        // ... varargs ignored
    ) -> u32 {
        // Return empty string – varargs are too complex to handle in a stub
        use crate::frameworks::foundation::ns_string::get_static_str;
        let empty = get_static_str(env, "");
        empty.to_bits()
    }
    return Some(&(fake_CFStringCreateWithFormat as fn(&mut crate::Environment, u32, u32, crate::mem::ConstPtr<u8>) -> u32));
}

if symbol == "_CFStringCreateMutable" {
    fn fake_CFStringCreateMutable(_env: &mut crate::Environment, _alloc: u32, _maxLength: u32) -> u32 {
        0 // nil (we don't support mutable CFStrings)
    }
    return Some(&(fake_CFStringCreateMutable as fn(&mut crate::Environment, u32, u32) -> u32));
}

if symbol == "_CFStringCreateMutableCopy" {
    fn fake_CFStringCreateMutableCopy(_env: &mut crate::Environment, _alloc: u32, _maxLength: u32, _theString: u32) -> u32 {
        0
    }
    return Some(&(fake_CFStringCreateMutableCopy as fn(&mut crate::Environment, u32, u32, u32) -> u32));
}

if symbol == "_CFStringCreateCopy" {
    fn fake_CFStringCreateCopy(env: &mut crate::Environment, _alloc: u32, theString: u32) -> u32 {
        use crate::objc::retain;
        let nsstr = crate::objc::id::from_bits(theString);
        retain(env, nsstr);
        theString
    }
    return Some(&(fake_CFStringCreateCopy as fn(&mut crate::Environment, u32, u32) -> u32));
}

if symbol == "_CFStringCreateArrayBySeparatingStrings" {
    fn fake_CFStringCreateArrayBySeparatingStrings(_env: &mut crate::Environment, _alloc: u32, _string: u32, _separator: u32) -> u32 {
        // Return empty array
        use crate::objc::msg_class;
        let arr: u32 = msg_class![_env; NSArray array];
        arr
    }
    return Some(&(fake_CFStringCreateArrayBySeparatingStrings as fn(&mut crate::Environment, u32, u32, u32) -> u32));
}

if symbol == "_CFStringCreateWithSubstring" {
    fn fake_CFStringCreateWithSubstring(
        env: &mut crate::Environment,
        _alloc: u32,
        string: u32,
        _range: u64, // CFRange packed into u64
    ) -> u32 {
        // Return the original string (simplistic)
        string
    }
    return Some(&(fake_CFStringCreateWithSubstring as fn(&mut crate::Environment, u32, u32, u64) -> u32));
}

if symbol == "_CFStringGetBytes" {
    fn fake_CFStringGetBytes(
        env: &mut crate::Environment,
        theString: u32,
        _range: u64,
        _encoding: u32,
        _lossByte: u8,
        _buffer: crate::mem::MutPtr<u8>,
        _maxBufLen: u32,
        _usedBufLen: crate::mem::MutPtr<u32>,
    ) -> u32 {
        use crate::frameworks::foundation::ns_string::to_rust_string;
        let nsstr = crate::mem::MutPtr::from_bits(theString);
        let s = to_rust_string(env, nsstr);
        s.len() as u32
    }
    return Some(&(fake_CFStringGetBytes as fn(&mut crate::Environment, u32, u64, u32, u8, crate::mem::MutPtr<u8>, u32, crate::mem::MutPtr<u32>) -> u32));
}

if symbol == "_CFStringGetCString" {
    fn fake_CFStringGetCString(
        env: &mut crate::Environment,
        theString: u32,
        buffer: crate::mem::MutPtr<u8>,
        bufferSize: u32,
        _encoding: u32,
    ) -> bool {
        use crate::frameworks::foundation::ns_string::to_rust_string;
        let nsstr = crate::mem::MutPtr::from_bits(theString);
        let s = to_rust_string(env, nsstr);
        let bytes = s.as_bytes();
        let copy_len = bufferSize.min(bytes.len() as u32);
        for i in 0..copy_len {
            env.mem.write(buffer + i, bytes[i as usize]);
        }
        if copy_len < bufferSize {
            env.mem.write(buffer + copy_len, 0);
        }
        true
    }
    return Some(&(fake_CFStringGetCString as fn(&mut crate::Environment, u32, crate::mem::MutPtr<u8>, u32, u32) -> bool));
}

if symbol == "_CFStringGetCStringPtr" {
    fn fake_CFStringGetCStringPtr(
        env: &mut crate::Environment,
        theString: u32,
        _encoding: u32,
    ) -> u32 {
        // Return NULL pointer – caller should fall back to GetCString
        0
    }
    return Some(&(fake_CFStringGetCStringPtr as fn(&mut crate::Environment, u32, u32) -> u32));
}

if symbol == "_CFStringGetFastestEncoding" {
    fn fake_CFStringGetFastestEncoding(_env: &mut crate::Environment, _theString: u32) -> u32 {
        0x08000100 // kCFStringEncodingUTF8
    }
    return Some(&(fake_CFStringGetFastestEncoding as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_CFStringGetMaximumSizeForEncoding" {
    fn fake_CFStringGetMaximumSizeForEncoding(_env: &mut crate::Environment, _length: u32, _encoding: u32) -> u32 {
        _length * 4 // worst-case UTF-8
    }
    return Some(&(fake_CFStringGetMaximumSizeForEncoding as fn(&mut crate::Environment, u32, u32) -> u32));
}

if symbol == "_CFStringCompare" {
    fn fake_CFStringCompare(
        env: &mut crate::Environment,
        str1: u32,
        str2: u32,
        _options: u32,
    ) -> u32 {
        use crate::frameworks::foundation::ns_string::to_rust_string;
        let s1 = to_rust_string(env, crate::mem::MutPtr::from_bits(str1));
        let s2 = to_rust_string(env, crate::mem::MutPtr::from_bits(str2));
        match s1.cmp(&s2) {
            std::cmp::Ordering::Less => -1i32 as u32,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }
    return Some(&(fake_CFStringCompare as fn(&mut crate::Environment, u32, u32, u32) -> u32));
}

if symbol == "_CFStringHasPrefix" {
    fn fake_CFStringHasPrefix(
        env: &mut crate::Environment,
        str: u32,
        prefix: u32,
    ) -> bool {
        use crate::frameworks::foundation::ns_string::to_rust_string;
        let s = to_rust_string(env, crate::objc::id::from_bits(str));
        let p = to_rust_string(env, crate::objc::id::from_bits(prefix));
        s.starts_with(&*p)
    }
    return Some(&(fake_CFStringHasPrefix as fn(&mut crate::Environment, u32, u32) -> bool));
}

if symbol == "_CFStringHasSuffix" {
    fn fake_CFStringHasSuffix(
        env: &mut crate::Environment,
        str: u32,
        suffix: u32,
    ) -> bool {
        use crate::frameworks::foundation::ns_string::to_rust_string;
        let s = to_rust_string(env, crate::objc::id::from_bits(str));
        let suf = to_rust_string(env, crate::objc::id::from_bits(suffix));
        s.ends_with(&*suf)
    }
    return Some(&(fake_CFStringHasSuffix as fn(&mut crate::Environment, u32, u32) -> bool));
}

if symbol == "_CFStringFind" {
    fn fake_CFStringFind(
        env: &mut crate::Environment,
        str: u32,
        substr: u32,
        _options: u32,
    ) -> u64 {
        use crate::frameworks::foundation::ns_string::to_rust_string;
        let s = to_rust_string(env, crate::objc::id::from_bits(str));
        let sub = to_rust_string(env, crate::objc::id::from_bits(substr));
        if let Some(pos) = s.find(&*sub) {
            (pos as u64) << 32 | (sub.len() as u64)
        } else {
            0x7fffffff00000000
        }
    }
    return Some(&(fake_CFStringFind as fn(&mut crate::Environment, u32, u32, u32) -> u64));
}

if symbol == "_CFStringLowercase" {
    fn fake_CFStringLowercase(
        env: &mut crate::Environment,
        theString: u32,
        _locale: u32,
    ) -> u32 {
        use crate::frameworks::foundation::ns_string::{from_rust_string, to_rust_string};
        let s = to_rust_string(env, crate::mem::MutPtr::from_bits(theString));
        let lower = s.to_lowercase();
        let nsstr = from_rust_string(env, lower);
        nsstr.to_bits()
    }
    return Some(&(fake_CFStringLowercase as fn(&mut crate::Environment, u32, u32) -> u32));
}

if symbol == "_CFStringAppendCharacters" {
    fn fake_CFStringAppendCharacters(_env: &mut crate::Environment, _theString: u32, _chars: u32, _numChars: u32) {
        // no-op (mutable strings not supported)
    }
    return Some(&(fake_CFStringAppendCharacters as fn(&mut crate::Environment, u32, u32, u32) -> ()));
}

if symbol == "_CFStringAppendFormat" {
    fn fake_CFStringAppendFormat(_env: &mut crate::Environment, _theString: u32, _formatOptions: u32, _format: u32) {
        // no-op
    }
    return Some(&(fake_CFStringAppendFormat as fn(&mut crate::Environment, u32, u32, u32) -> ()));
}

if symbol == "_CFStringConvertEncodingToNSStringEncoding" {
    fn fake_CFStringConvertEncodingToNSStringEncoding(_env: &mut crate::Environment, _encoding: u32) -> u32 {
        0x08000100 // NSUTF8StringEncoding
    }
    return Some(&(fake_CFStringConvertEncodingToNSStringEncoding as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_CFStringConvertNSStringEncodingToEncoding" {
    fn fake_CFStringConvertNSStringEncodingToEncoding(_env: &mut crate::Environment, _nsEncoding: u32) -> u32 {
        0x08000100
    }
    return Some(&(fake_CFStringConvertNSStringEncodingToEncoding as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_CFStringConvertEncodingToIANACharSetName" {
    fn fake_CFStringConvertEncodingToIANACharSetName(env: &mut crate::Environment, _encoding: u32) -> u32 {
        use crate::frameworks::foundation::ns_string::get_static_str;
        let name = get_static_str(env, "utf-8");
        name.to_bits()
    }
    return Some(&(fake_CFStringConvertEncodingToIANACharSetName as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_CFStringConvertIANACharSetNameToEncoding" {
    fn fake_CFStringConvertIANACharSetNameToEncoding(_env: &mut crate::Environment, _charsetName: u32) -> u32 {
        0x08000100 // UTF-8 encoding constant
    }
    return Some(&(fake_CFStringConvertIANACharSetNameToEncoding as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_CFURLCopyFileSystemPath" {
    fn fake_CFURLCopyFileSystemPath(env: &mut crate::Environment, _url: u32, _pathStyle: u32) -> u32 {
        use crate::frameworks::foundation::ns_string::get_static_str;
        let path = get_static_str(env, "/");
        path.to_bits()
    }
    return Some(&(fake_CFURLCopyFileSystemPath as fn(&mut crate::Environment, u32, u32) -> u32));
}

if symbol == "_CFURLCopyPassword" {
    fn fake_CFURLCopyPassword(_env: &mut crate::Environment, _url: u32) -> u32 {
        0 // nil
    }
    return Some(&(fake_CFURLCopyPassword as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_CFURLCopyUserName" {
    fn fake_CFURLCopyUserName(_env: &mut crate::Environment, _url: u32) -> u32 {
        0
    }
    return Some(&(fake_CFURLCopyUserName as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_CFURLCreateStringByAddingPercentEscapes" {
    fn fake_CFURLCreateStringByAddingPercentEscapes(
        env: &mut crate::Environment,
        _allocator: u32,
        originalString: u32,
        _charactersToLeaveUnescaped: u32,
        _legalURLCharactersToBeEscaped: u32,
        _encoding: u32,
    ) -> u32 {
        // Return the original string unmodified
        originalString
    }
    return Some(&(fake_CFURLCreateStringByAddingPercentEscapes as fn(&mut crate::Environment, u32, u32, u32, u32, u32) -> u32));
}

if symbol == "_CFURLCreateStringByReplacingPercentEscapesUsingEncoding" {
    fn fake_CFURLCreateStringByReplacingPercentEscapesUsingEncoding(
        env: &mut crate::Environment,
        _allocator: u32,
        originalString: u32,
        _charactersToLeaveEscaped: u32,
        _encoding: u32,
    ) -> u32 {
        originalString
    }
    return Some(&(fake_CFURLCreateStringByReplacingPercentEscapesUsingEncoding as fn(&mut crate::Environment, u32, u32, u32, u32) -> u32));
}
if symbol == "_dispatch_time" {
    fn dispatch_time_stub(_env: &mut Environment, when: u64, delta: i64) -> u64 {
        log_dbg!("_dispatch_time stub called, when={}, delta={}", when, delta);
        // If when is DISPATCH_TIME_NOW (0), return current time + delta.
        // Otherwise just return when + delta as a dummy value.
        (when as i64 + delta) as u64
    }
    return Some(&(dispatch_time_stub as fn(&mut Environment, u64, i64) -> u64));
}
if symbol == "_dispatch_after" {
    fn dispatch_after_stub(_env: &mut Environment, when: u64, delta: i64) -> u64 {
        log_dbg!("_dispatch_after stub called, when={}, delta={}", when, delta);
        // If when is DISPATCH_TIME_NOW (0), return current time + delta.
        // Otherwise just return when + delta as a dummy value.
        (when as i64 + delta) as u64
    }
    return Some(&(dispatch_after_stub as fn(&mut Environment, u64, i64) -> u64));
}
if symbol == "_sigaction" {
    fn fake_sigaction(
        _env: &mut crate::Environment,
        _sig: i32,
        _act: u32,
        _oact: u32,
    ) -> i32 {
        log_dbg!("_sigaction stub called, returning 0 (success)");
        0
    }
    return Some(&(fake_sigaction as fn(&mut crate::Environment, i32, u32, u32) -> i32));
}

if symbol == "_sigaltstack" {
    fn fake_sigaltstack(
        _env: &mut crate::Environment,
        _ss: u32,
        _oss: u32,
    ) -> i32 {
        log_dbg!("_sigaltstack stub called, returning 0 (success)");
        0
    }
    return Some(&(fake_sigaltstack as fn(&mut crate::Environment, u32, u32) -> i32));
}
if symbol == "_objc_retain" {
    fn fake_objc_retain(_env: &mut crate::Environment, obj: u32) -> u32 {
        // Return the same object (retain is a no-op in our simplified runtime)
        obj
    }
    return Some(&(fake_objc_retain as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_objc_release" {
    fn fake_objc_release(_env: &mut crate::Environment, _obj: u32) {
        // No-op
    }
    return Some(&(fake_objc_release as fn(&mut crate::Environment, u32) -> ()));
}

if symbol == "_objc_autorelease" {
    fn fake_objc_autorelease(_env: &mut crate::Environment, obj: u32) -> u32 {
        obj
    }
    return Some(&(fake_objc_autorelease as fn(&mut crate::Environment, u32) -> u32));
}
if symbol == "_objc_retainAutoreleasedReturnValue" {
    fn fake_objc_retain_autoreleased_return_value(_env: &mut crate::Environment, obj: u32) -> u32 {
        obj
    }
    return Some(&(fake_objc_retain_autoreleased_return_value as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_setvbuf" {
    fn fake_setvbuf(_env: &mut crate::Environment, _stream: u32, _buf: u32, _mode: i32, _size: u32) -> i32 {
        log_dbg!("_setvbuf stub called, returning 0 (success)");
        0
    }
    return Some(&(fake_setvbuf as fn(&mut crate::Environment, u32, u32, i32, u32) -> i32));
}

if symbol == "_setxattr" {
    fn fake_setxattr(_env: &mut crate::Environment, _path: u32, _name: u32, _value: u32, _size: u32, _position: u32, _flags: i32) -> i32 {
        log_dbg!("_setxattr stub called, returning 0 (success)");
        0
    }
    return Some(&(fake_setxattr as fn(&mut crate::Environment, u32, u32, u32, u32, u32, i32) -> i32));
}

if symbol == "_objc_autoreleaseReturnValue" {
    fn fake_objc_autorelease_return_value(_env: &mut crate::Environment, obj: u32) -> u32 {
        obj
    }
    return Some(&(fake_objc_autorelease_return_value as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_objc_retainAutorelease" {
    fn fake_objc_retain_autorelease(_env: &mut crate::Environment, obj: u32) -> u32 {
        obj
    }
    return Some(&(fake_objc_retain_autorelease as fn(&mut crate::Environment, u32) -> u32));
}

if symbol == "_objc_storeStrong" {
    fn fake_objc_store_strong(_env: &mut crate::Environment, _ptr: u32, _obj: u32) {
        // No-op
    }
    return Some(&(fake_objc_store_strong as fn(&mut crate::Environment, u32, u32) -> ()));
}
        if symbol == "_objc_autoreleasePoolPush" {
            fn fake_pool_push(_env: &mut crate::Environment) -> u32 {
                // Return a dummy pool token so the game thinks it created a memory pool
                0xDEADBEEF
            }
            return Some(&(fake_pool_push as fn(&mut crate::Environment) -> u32));
        }

        if symbol == "_objc_autoreleasePoolPop" {
            fn fake_pool_pop(_env: &mut crate::Environment, _token: u32) {
                // Safely ignore the pop request so it doesn't crash trying to free the dummy token!
            }
            return Some(&(fake_pool_pop as fn(&mut crate::Environment, u32) -> ()));
        }
        if symbol == "__setjmp" || symbol == "_setjmp" {
            fn fake_setjmp(_env: &mut crate::Environment, _jmp_buf: u32) -> u32 {
                println!("🎮 LOG: Stubbed setjmp exception handler! Returning 0.");
                // Returning 0 tells the C++ engine we are executing the try-block normally.
                0 
            }
            return Some(&(fake_setjmp as fn(&mut crate::Environment, u32) -> u32));
        }

        if symbol == "_longjmp" || symbol == "__longjmp" || symbol == "longjmp" {
            fn fake_longjmp(_env: &mut crate::Environment, _jmp_buf: u32, _val: u32) {
                // If the game actually tries to throw an exception, we will catch it here.
                println!("🎮 LOG: Game attempted to longjmp (throw an exception)! Ignoring.");
            }
            return Some(&(fake_longjmp as fn(&mut crate::Environment, u32, u32) -> ()));
        }
        if symbol == "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6__initEPKcm" {
            fn fake_libcxx_string_init(
                env: &mut crate::Environment,
                this_ptr: crate::mem::MutPtr<u8>,
                _str_src: crate::mem::ConstPtr<u8>,
                _size: u32,
            ) -> crate::mem::MutPtr<u8> {
                println!("🎮 LOG: Caught C++ std::string::__init! Faking safe string memory.");
                env.mem.write(this_ptr, 0u8);
                this_ptr
            }
            return Some(
                &(fake_libcxx_string_init
                    as fn(
                        &mut crate::Environment,
                        crate::mem::MutPtr<u8>,
                        crate::mem::ConstPtr<u8>,
                        u32,
                    ) -> crate::mem::MutPtr<u8>),
            );
        }

        // ==========================================================
        // 🏎️ MODERN C++ BYPASS: std::string Copy Constructor
        // ==========================================================
        if symbol == "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEC1ERKS5_" {
            fn fake_libcxx_string_copy(
                env: &mut crate::Environment,
                this_ptr: crate::mem::MutPtr<u8>,
                _other_ptr: crate::mem::ConstPtr<u8>,
            ) -> crate::mem::MutPtr<u8> {
                println!("🎮 LOG: Caught C++ std::string copy constructor! Faking safe string memory.");
                env.mem.write(this_ptr, 0u8);
                this_ptr
            }
            return Some(
                &(fake_libcxx_string_copy
                    as fn(
                        &mut crate::Environment,
                        crate::mem::MutPtr<u8>,
                        crate::mem::ConstPtr<u8>,
                    ) -> crate::mem::MutPtr<u8>),
            );
        }

        panic!("Call to unimplemented function {symbol}");
        log!("WARNING: unimplemented function {symbol}, using stub");

        fn dummy(env: &mut crate::Environment) {
            env.cpu.regs_mut()[0] = 0;
        }

        let f: HostFunction = &(dummy as fn(&mut crate::Environment) -> ());
        Some(f)
    }

    /// Creates a guest function that will call a host function with the name
    /// `symbol`. This can be used to implement "get proc address" functions.
    /// Note that no attempt is made to deduplicate or deallocate these, so
    /// excessive use would create a memory leak.
    ///
    /// The name must be the mangled symbol name. Returns [Err] if there's no
    /// such function.
    pub fn create_proc_address(
        &mut self,
        mem: &mut Mem,
        cpu: &mut Cpu,
        symbol: &str,
    ) -> Result<GuestFunction, ()> {
        let function_ptr = self.create_proc_address_no_inval(mem, symbol)?;

        // Just in case
        cpu.invalidate_cache_range(function_ptr.addr_without_thumb_bit(), 8);
        Ok(function_ptr)
    }

    /// Internal [Self::create_proc_address] that doesn't invalidate the cache.
    /// For use before a [Cpu] is available.
    fn create_proc_address_no_inval(
        &mut self,
        mem: &mut Mem,
        symbol: &str,
    ) -> Result<GuestFunction, ()> {
        let &(symbol, f) = search_host_dylibs(|dylib| dylib.function_exports, symbol).ok_or(())?;
        if let Some(&cached_fn) = self.non_lazy_host_functions.get(symbol) {
            return Ok(cached_fn);
        }
        let function_ptr = self.create_guest_function(mem, symbol, f);
        self.non_lazy_host_functions.insert(symbol, function_ptr);
        Ok(function_ptr)
    }

    pub fn create_guest_function(
        &mut self,
        mem: &mut Mem, // 🏎️ REVERT: Back to 'mem: &mut Mem' to appease the Borrow Checker!
        symbol: &'static str,
        f: HostFunction,
    ) -> GuestFunction {
        // Allocate an SVC ID for this host function
        let idx: u32 = self.linked_host_functions.len().try_into().unwrap();
        let svc = idx + Self::SVC_LINKED_FUNCTIONS_BASE;
        self.linked_host_functions.push((symbol, f));

        // Create guest function to call this host function
        let function_ptr = mem.alloc(8);
        let ptr_slice = mem.bytes_at_mut(function_ptr.cast::<u8>(), 8);

        // ==========================================================
        // 🏎️ AARCH64 DYNAMIC STUB GENERATION
        // ==========================================================
        if self.is_64_bit { // 🏎️ Check Dyld's internal flag!
            // AArch64 SVC instruction: 0xD4000001 | (imm16 << 5)
            let svc_inst: u32 = 0xd4000001 | ((svc & 0xffff) << 5);
            // AArch64 RET instruction: 0xD65F03C0
            let ret_inst: u32 = 0xd65f03c0; 
            
            ptr_slice[0..4].copy_from_slice(&svc_inst.to_le_bytes());
            ptr_slice[4..8].copy_from_slice(&ret_inst.to_le_bytes());
        } else {
            // 🏎️ FIX: ARMv7 Unconditional SVC is 0xEF000000, NOT 0xDF000000
            let svc_inst: u32 = 0xef000000 | (svc & 0xffffff); 
            let bx_lr_inst: u32 = 0xe12fff1e;
            
            ptr_slice[0..4].copy_from_slice(&svc_inst.to_le_bytes());
            ptr_slice[4..8].copy_from_slice(&bx_lr_inst.to_le_bytes());
        }
        GuestFunction::from_addr_with_thumb_bit(function_ptr.to_bits())
    }
}
