/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! Centralized bypass for dead game servers and tracking services.

pub struct NetBypass;

impl NetBypass {
    /// Returns true if the domain is known to be dead or cause hangs.
    pub fn is_blocked_domain(domain: &str) -> bool {
        let blocked_list = [
            "gameloft.com",
            "vgold.gameloft.com",
            "gllive.gameloft.com",
            "livewebapp.gameloft.com",
            "ets.gameloft.com",
            "ma.mkhoj.com",
            "admob.com",
            "google-analytics.com",
            "flurry.com",
            "tapjoy.com",
            "facebook.com",
        ];

        let domain_lower = domain.to_lowercase();
        blocked_list.iter().any(|&d| domain_lower.contains(d))
    }

    /// Returns the errno for "Network Unreachable".
    pub fn get_offline_errno() -> i32 {
        101 // ENETUNREACH
    }
}