/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <thread>
#include <array>
#include <optional>
#ifdef __ANDROID__
#include <android/log.h>
#endif

#include "dynarmic/interface/A32/a32.h"
#include "dynarmic/interface/A32/config.h"
#include "dynarmic/interface/A32/coprocessor.h"
#include "dynarmic/interface/exclusive_monitor.h"

#include "dynarmic/interface/A64/a64.h"
#include "dynarmic/interface/A64/config.h"

namespace touchHLE::cpu {

// 🏎️ RUST FFI IMPORTS
// We force 64-bit addresses here so it perfectly matches Rust's aarch64 feature!
extern "C" {
  struct touchHLE_Mem;
  std::uint8_t touchHLE_cpu_read_u8(touchHLE_Mem *mem, std::uint64_t addr, bool *error);
  std::uint16_t touchHLE_cpu_read_u16(touchHLE_Mem *mem, std::uint64_t addr, bool *error);
  std::uint32_t touchHLE_cpu_read_u32(touchHLE_Mem *mem, std::uint64_t addr, bool *error);
  std::uint64_t touchHLE_cpu_read_u64(touchHLE_Mem *mem, std::uint64_t addr, bool *error);
  bool touchHLE_cpu_write_u8(touchHLE_Mem *mem, std::uint64_t addr, std::uint8_t value);
  bool touchHLE_cpu_write_u16(touchHLE_Mem *mem, std::uint64_t addr, std::uint16_t value);
  bool touchHLE_cpu_write_u32(touchHLE_Mem *mem, std::uint64_t addr, std::uint32_t value);
  bool touchHLE_cpu_write_u64(touchHLE_Mem *mem, std::uint64_t addr, std::uint64_t value);

  struct touchHLE_DynarmicContext {
    std::array<std::uint32_t, 16> regs;
    std::array<std::uint32_t, 64> extregs;
    std::uint32_t cpsr;
    std::uint32_t fpscr;
  };
}

const auto HaltReasonSvc = Dynarmic::HaltReason::UserDefined1;
const auto HaltReasonUndefinedInstruction = Dynarmic::HaltReason::UserDefined2;
const auto HaltReasonBreakpoint = Dynarmic::HaltReason::UserDefined3;

// ==========================================================
// 🏎️ 32-BIT (A32) IMPLEMENTATION
// ==========================================================
class Environment final : public Dynarmic::A32::UserCallbacks {
public:
  Dynarmic::A32::Jit *cpu = nullptr;
  touchHLE_Mem *mem = nullptr;
  std::uint64_t ticks_remaining;
  uint32_t halting_svc;

private:
  std::uint8_t MemoryRead8(std::uint32_t vaddr) override {
    bool error;
    auto value = touchHLE_cpu_read_u8(mem, vaddr, &error);
    if (error) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    return value;
  }
  std::uint16_t MemoryRead16(std::uint32_t vaddr) override {
    bool error;
    auto value = touchHLE_cpu_read_u16(mem, vaddr, &error);
    if (error) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    return value;
  }
  std::uint32_t MemoryRead32(std::uint32_t vaddr) override {
    bool error;
    auto value = touchHLE_cpu_read_u32(mem, vaddr, &error);
    if (error) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    return value;
  }
  std::uint64_t MemoryRead64(std::uint32_t vaddr) override {
    bool error;
    auto value = touchHLE_cpu_read_u64(mem, vaddr, &error);
    if (error) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    return value;
  }

  std::optional<std::uint32_t> MemoryReadCode(std::uint32_t vaddr) override {
    bool error;
    auto value = touchHLE_cpu_read_u32(mem, vaddr, &error);
    if (error) return std::nullopt;
    return value;
  }

  void MemoryWrite8(std::uint32_t vaddr, std::uint8_t value) override {
    if (touchHLE_cpu_write_u8(mem, vaddr, value)) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
  }
  void MemoryWrite16(std::uint32_t vaddr, std::uint16_t value) override {
    if (touchHLE_cpu_write_u16(mem, vaddr, value)) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
  }
  void MemoryWrite32(std::uint32_t vaddr, std::uint32_t value) override {
    if (touchHLE_cpu_write_u32(mem, vaddr, value)) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
  }
  void MemoryWrite64(std::uint32_t vaddr, std::uint64_t value) override {
    if (touchHLE_cpu_write_u64(mem, vaddr, value)) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
  }

  bool MemoryWriteExclusive8(std::uint32_t addr, std::uint8_t value, std::uint8_t expected) override {
    if (MemoryRead8(addr) != expected) return false;
    MemoryWrite8(addr, value);
    return true;
  }
  bool MemoryWriteExclusive16(std::uint32_t addr, std::uint16_t value, std::uint16_t expected) override {
    if (MemoryRead16(addr) != expected) return false;
    MemoryWrite16(addr, value);
    return true;
  }
  bool MemoryWriteExclusive32(std::uint32_t addr, std::uint32_t value, std::uint32_t expected) override {
    if (MemoryRead32(addr) != expected) return false;
    MemoryWrite32(addr, value);
    return true;
  }
  bool MemoryWriteExclusive64(std::uint32_t addr, std::uint64_t value, std::uint64_t expected) override {
    if (MemoryRead64(addr) != expected) return false;
    MemoryWrite64(addr, value);
    return true;
  }

  void InterpreterFallback(std::uint32_t, size_t) override { abort(); }
  void CallSVC(std::uint32_t svc) override {
    halting_svc = svc;
    cpu->HaltExecution(HaltReasonSvc);
  }
  void ExceptionRaised(std::uint32_t pc, Dynarmic::A32::Exception exception) override {
    if (exception == Dynarmic::A32::Exception::NoExecuteFault) {
      cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    } else if (exception == Dynarmic::A32::Exception::UndefinedInstruction) {
      if ((cpu->Cpsr() & 0x20) == 0) {
        cpu->SetCpsr(cpu->Cpsr() | 0x20);
        return;
      }
      cpu->HaltExecution(HaltReasonUndefinedInstruction);
    } else if (exception == Dynarmic::A32::Exception::Breakpoint) {
      cpu->HaltExecution(HaltReasonBreakpoint);
    } else {
      cpu->HaltExecution(HaltReasonUndefinedInstruction);
    }
  }
  void AddTicks(std::uint64_t ticks) override {
    if (ticks > ticks_remaining) {
      ticks_remaining = 0;
      return;
    }
    ticks_remaining -= ticks;
  }
  std::uint64_t GetTicksRemaining() override { return ticks_remaining; }
};

class ArmDynarmicCP15 : public Dynarmic::A32::Coprocessor {
  std::uint32_t addr = 0;
public:
  using CoprocReg = Dynarmic::A32::CoprocReg;
  CallbackOrAccessOneWord CompileSendOneWord(bool two, unsigned opc1, CoprocReg CRn, CoprocReg CRm, unsigned opc2) override {
    if (!two && CRn == CoprocReg::C7 && opc1 == 0 && CRm == CoprocReg::C10 && opc2 == 5) return &addr;
    return CallbackOrAccessOneWord{};
  }
  std::optional<Callback> CompileInternalOperation(bool, unsigned, CoprocReg, CoprocReg, CoprocReg, unsigned) override { return std::nullopt; }
  CallbackOrAccessTwoWords CompileSendTwoWords(bool, unsigned, CoprocReg) override { return CallbackOrAccessTwoWords{}; }
  CallbackOrAccessOneWord CompileGetOneWord(bool, unsigned, CoprocReg, CoprocReg, unsigned) override { return CallbackOrAccessOneWord{}; }
  CallbackOrAccessTwoWords CompileGetTwoWords(bool, unsigned, CoprocReg) override { return CallbackOrAccessTwoWords{}; }
  std::optional<Callback> CompileLoadWords(bool, bool, CoprocReg, std::optional<std::uint8_t>) override { return std::nullopt; }
  std::optional<Callback> CompileStoreWords(bool, bool, CoprocReg, std::optional<std::uint8_t>) override { return std::nullopt; }
};

class DynarmicWrapper {
  Environment env;
  std::unique_ptr<Dynarmic::A32::Jit> cpu;
  std::unique_ptr<Dynarmic::ExclusiveMonitor> mon;
  std::array<std::uint8_t *, Dynarmic::A32::UserConfig::NUM_PAGE_TABLE_ENTRIES> page_table;
public:
  DynarmicWrapper(void *direct_memory_access_ptr, size_t null_page_count) {
    setvbuf(stdout, nullptr, _IONBF, 0);
    setvbuf(stderr, nullptr, _IONBF, 0);
    Dynarmic::A32::UserConfig user_config;
    user_config.callbacks = &env;
    user_config.coprocessors[15] = std::make_shared<ArmDynarmicCP15>();
    mon = std::make_unique<Dynarmic::ExclusiveMonitor>(1);
    user_config.global_monitor = mon.get();
    user_config.check_halt_on_memory_access = true;
    if (direct_memory_access_ptr) {
      page_table.fill((std::uint8_t *)direct_memory_access_ptr);
      if (null_page_count > page_table.size()) abort();
      for (size_t i = 0; i < null_page_count; i++) page_table[i] = nullptr;
      user_config.page_table = &page_table;
      user_config.absolute_offset_page_table = true;
    }
    cpu = std::make_unique<Dynarmic::A32::Jit>(user_config);
    env.cpu = cpu.get();
  }
  const std::uint32_t *regs() const { return &cpu->Regs().front(); }
  std::uint32_t *regs() { return &cpu->Regs().front(); }
  std::uint32_t cpsr() const { return cpu->Cpsr(); }
  void set_cpsr(std::uint32_t cpsr) { cpu->SetCpsr(cpsr); }
  void invalidate_cache_range(std::uint32_t start, std::uint32_t size) { cpu->InvalidateCacheRange(start, size); }
  void swap_context(touchHLE_DynarmicContext *context) {
    touchHLE_DynarmicContext tmp = {cpu->Regs(), cpu->ExtRegs(), cpu->Cpsr(), cpu->Fpscr()};
    cpu->Regs() = context->regs;
    cpu->ExtRegs() = context->extregs;
    cpu->SetCpsr(context->cpsr);
    cpu->SetFpscr(context->fpscr);
    *context = tmp;
  }
  std::int32_t run_or_step(touchHLE_Mem *mem, std::uint64_t *ticks) {
    env.mem = mem;
    Dynarmic::HaltReason hr;
    if (ticks) {
      env.ticks_remaining = *ticks;
      hr = cpu->Run();
    } else hr = cpu->Step();
    std::int32_t res;
    if ((!hr && ticks) || (hr == Dynarmic::HaltReason::Step && !ticks)) res = -1;
    else if (Dynarmic::Has(hr, Dynarmic::HaltReason::MemoryAbort)) res = -2;
    else if (Dynarmic::Has(hr, HaltReasonUndefinedInstruction)) res = -3;
    else if (Dynarmic::Has(hr, HaltReasonBreakpoint)) res = -4;
    else if (Dynarmic::Has(hr, HaltReasonSvc)) res = std::int32_t(env.halting_svc);
    else abort();
    env.mem = nullptr;
    if (ticks) *ticks = env.ticks_remaining;
    return res;
  }
};

// ==========================================================
// 🏎️ 64-BIT (A64) IMPLEMENTATION
// ==========================================================
class EnvironmentA64 final : public Dynarmic::A64::UserCallbacks {
public:
  Dynarmic::A64::Jit *cpu = nullptr;
  touchHLE_Mem *mem = nullptr;
  std::uint64_t ticks_remaining;
  uint32_t halting_svc;

private:
  std::uint8_t MemoryRead8(std::uint64_t vaddr) override {
    bool error = false;
    auto value = touchHLE_cpu_read_u8(mem, vaddr, &error);
    if (error) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    return value;
  }
  std::uint16_t MemoryRead16(std::uint64_t vaddr) override {
    bool error = false;
    auto value = touchHLE_cpu_read_u16(mem, vaddr, &error);
    if (error) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    return value;
  }
  std::uint32_t MemoryRead32(std::uint64_t vaddr) override {
    bool error = false;
    auto value = touchHLE_cpu_read_u32(mem, vaddr, &error);
    if (error) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    return value;
  }
  std::uint64_t MemoryRead64(std::uint64_t vaddr) override {
    bool error = false;
    auto value = touchHLE_cpu_read_u64(mem, vaddr, &error);
    if (error) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    return value;
  }
  Dynarmic::A64::Vector MemoryRead128(std::uint64_t vaddr) override {
    bool err1 = false, err2 = false;
    std::uint64_t lo = touchHLE_cpu_read_u64(mem, vaddr, &err1);
    std::uint64_t hi = touchHLE_cpu_read_u64(mem, vaddr + 8, &err2);
    if (err1 || err2) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    return {lo, hi};
  }

  std::optional<std::uint32_t> MemoryReadCode(std::uint64_t vaddr) override {
    bool error = false;
    auto value = touchHLE_cpu_read_u32(mem, vaddr, &error);
    if (error) return std::nullopt;
    return value;
  }

  void MemoryWrite8(std::uint64_t vaddr, std::uint8_t value) override {
    if (touchHLE_cpu_write_u8(mem, vaddr, value)) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
  }
  void MemoryWrite16(std::uint64_t vaddr, std::uint16_t value) override {
    if (touchHLE_cpu_write_u16(mem, vaddr, value)) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
  }
  void MemoryWrite32(std::uint64_t vaddr, std::uint32_t value) override {
    if (touchHLE_cpu_write_u32(mem, vaddr, value)) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
  }
  void MemoryWrite64(std::uint64_t vaddr, std::uint64_t value) override {
    if (touchHLE_cpu_write_u64(mem, vaddr, value)) cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
  }
  void MemoryWrite128(std::uint64_t vaddr, Dynarmic::A64::Vector value) override {
    if (touchHLE_cpu_write_u64(mem, vaddr, value[0]) || touchHLE_cpu_write_u64(mem, vaddr + 8, value[1])) {
      cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    }
  }

  bool MemoryWriteExclusive8(std::uint64_t vaddr, std::uint8_t value, std::uint8_t expected) override {
    if (MemoryRead8(vaddr) != expected) return false;
    MemoryWrite8(vaddr, value);
    return true;
  }
  bool MemoryWriteExclusive16(std::uint64_t vaddr, std::uint16_t value, std::uint16_t expected) override {
    if (MemoryRead16(vaddr) != expected) return false;
    MemoryWrite16(vaddr, value);
    return true;
  }
  bool MemoryWriteExclusive32(std::uint64_t vaddr, std::uint32_t value, std::uint32_t expected) override {
    if (MemoryRead32(vaddr) != expected) return false;
    MemoryWrite32(vaddr, value);
    return true;
  }
  bool MemoryWriteExclusive64(std::uint64_t vaddr, std::uint64_t value, std::uint64_t expected) override {
    if (MemoryRead64(vaddr) != expected) return false;
    MemoryWrite64(vaddr, value);
    return true;
  }
  bool MemoryWriteExclusive128(std::uint64_t vaddr, Dynarmic::A64::Vector value, Dynarmic::A64::Vector expected) override {
    auto current = MemoryRead128(vaddr);
    if (current[0] != expected[0] || current[1] != expected[1]) return false;
    MemoryWrite128(vaddr, value);
    return true;
  }

  void InterpreterFallback(std::uint64_t, size_t) override { abort(); }
  void CallSVC(std::uint32_t svc) override {
    halting_svc = svc;
    cpu->HaltExecution(HaltReasonSvc);
  }
  void ExceptionRaised(std::uint64_t pc, Dynarmic::A64::Exception exception) override {
    if (exception == Dynarmic::A64::Exception::NoExecuteFault) {
      cpu->HaltExecution(Dynarmic::HaltReason::MemoryAbort);
    } else if (exception == Dynarmic::A64::Exception::UndefinedInstruction) {
      cpu->HaltExecution(HaltReasonUndefinedInstruction);
    } else if (exception == Dynarmic::A64::Exception::Breakpoint) {
      cpu->HaltExecution(HaltReasonBreakpoint);
    } else {
      cpu->HaltExecution(HaltReasonUndefinedInstruction);
    }
  }
  void AddTicks(std::uint64_t ticks) override {
    if (ticks > ticks_remaining) {
      ticks_remaining = 0;
      return;
    }
    ticks_remaining -= ticks;
  }
  std::uint64_t GetTicksRemaining() override { return ticks_remaining; }
  std::uint64_t GetCNTPCT() override { return 0; }
};

class DynarmicWrapperA64 {
  EnvironmentA64 env;
  std::unique_ptr<Dynarmic::A64::Jit> cpu;
  std::unique_ptr<Dynarmic::ExclusiveMonitor> mon;
  std::array<std::uint64_t, 33> flat_regs;

public:
  DynarmicWrapperA64(void *direct_memory_access_ptr, size_t null_page_count) {
    Dynarmic::A64::UserConfig user_config;
    user_config.callbacks = &env;
    mon = std::make_unique<Dynarmic::ExclusiveMonitor>(1);
    user_config.global_monitor = mon.get();
    cpu = std::make_unique<Dynarmic::A64::Jit>(user_config);
    env.cpu = cpu.get();
    flat_regs.fill(0);
  }

  void sync_to_flat() {
    for (int i = 0; i < 31; ++i) flat_regs[i] = cpu->GetRegister(i);
    flat_regs[31] = cpu->GetSP();
    flat_regs[32] = cpu->GetPC();
  }

  void sync_from_flat() {
    for (int i = 0; i < 31; ++i) cpu->SetRegister(i, flat_regs[i]);
    cpu->SetSP(flat_regs[31]);
    cpu->SetPC(flat_regs[32]);
  }

  const std::uint64_t *regs_const() {
    sync_to_flat();
    return flat_regs.data();
  }
  
  std::uint64_t *regs_mut() {
    sync_to_flat();
    return flat_regs.data();
  }

  std::uint32_t pstate() const { return cpu->GetPstate(); }
  void set_pstate(std::uint32_t pstate) { cpu->SetPstate(pstate); }
};

// ==========================================================
// 🏎️ C EXPORTS FOR RUST
// ==========================================================
extern "C" {
  // --- A32 ---
  DynarmicWrapper *touchHLE_DynarmicWrapper_new(void *ptr, size_t count) { return new DynarmicWrapper(ptr, count); }
  void touchHLE_DynarmicWrapper_delete(DynarmicWrapper *cpu) { delete cpu; }
  const std::uint32_t *touchHLE_DynarmicWrapper_regs_const(const DynarmicWrapper *cpu) { return cpu->regs(); }
  std::uint32_t *touchHLE_DynarmicWrapper_regs_mut(DynarmicWrapper *cpu) { return cpu->regs(); }
  std::uint32_t touchHLE_DynarmicWrapper_cpsr(const DynarmicWrapper *cpu) { return cpu->cpsr(); }
  void touchHLE_DynarmicWrapper_set_cpsr(DynarmicWrapper *cpu, std::uint32_t cpsr) { cpu->set_cpsr(cpsr); }
  void touchHLE_DynarmicWrapper_swap_context(DynarmicWrapper *cpu, touchHLE_DynarmicContext *ctx) { cpu->swap_context(ctx); }
  void touchHLE_DynarmicWrapper_invalidate_cache_range(DynarmicWrapper *cpu, std::uint32_t start, std::uint32_t size) { cpu->invalidate_cache_range(start, size); }
  std::int32_t touchHLE_DynarmicWrapper_run_or_step(DynarmicWrapper *cpu, touchHLE_Mem *mem, std::uint64_t *ticks) { return cpu->run_or_step(mem, ticks); }

  // --- A64 ---
  void *touchHLE_DynarmicWrapper_new_a64(void *ptr, size_t count) { return new DynarmicWrapperA64(ptr, count); }
  const std::uint64_t *touchHLE_DynarmicWrapper_regs_const_a64(DynarmicWrapperA64 *cpu) { return cpu->regs_const(); }
  std::uint64_t *touchHLE_DynarmicWrapper_regs_mut_a64(DynarmicWrapperA64 *cpu) { return cpu->regs_mut(); }
  std::uint32_t touchHLE_DynarmicWrapper_pstate(const DynarmicWrapperA64 *cpu) { return cpu->pstate(); }
  void touchHLE_DynarmicWrapper_set_pstate(DynarmicWrapperA64 *cpu, std::uint32_t pstate) { cpu->set_pstate(pstate); }
}

} // namespace touchHLE::cpu
