//! The channel clock's arithmetic and the machine-wide monotonic time base (PLAN §5.1).
//!
//! `mono_ns()` reads a clock every process on the machine shares (macOS `CLOCK_UPTIME_RAW`, the
//! same count as `mach_absolute_time`; Linux `CLOCK_MONOTONIC`; Windows QPC), so the output
//! process's `T_k`, the renderers' begin-frame deadlines and `painted_at` stamps compare
//! directly. `T_k` is computed from `k` each time in `u128`, never accumulated.

/// Frame rate as an exact fraction (60000/1001, 50/1, 30000/1001, 25/1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rate {
    pub num: u32,
    pub den: u32,
}

impl Rate {
    pub const P5994: Rate = Rate {
        num: 60000,
        den: 1001,
    };
    pub const P50: Rate = Rate { num: 50, den: 1 };
    pub const P2997: Rate = Rate {
        num: 30000,
        den: 1001,
    };
    pub const P25: Rate = Rate { num: 25, den: 1 };

    /// `ceil(fps)`: CEF's free-running rate in mode B (60 for 59.94).
    pub fn ceil_fps(self) -> u32 {
        self.num.div_ceil(self.den)
    }

    /// One frame period in ns (rounded down).
    pub fn period_ns(self) -> u64 {
        (u64::from(self.den) * 1_000_000_000) / u64::from(self.num)
    }
}

/// `T_k = epoch + floor(k · den · 10⁹ / num)` ns.
pub fn t_k(epoch_ns: u64, k: u64, rate: Rate) -> u64 {
    let off = (u128::from(k) * u128::from(rate.den) * 1_000_000_000) / u128::from(rate.num);
    epoch_ns + off as u64
}

/// `tc(k) = U0 + floor(k · den · 10⁷ / num)` in 100 ns units.
pub fn tc_k(u0_100ns: i64, k: u64, rate: Rate) -> i64 {
    let off = (u128::from(k) * u128::from(rate.den) * 10_000_000) / u128::from(rate.num);
    u0_100ns + off as i64
}

/// The first slot whose deadline is at or after `now_ns`.
pub fn k_at_or_after(epoch_ns: u64, now_ns: u64, rate: Rate) -> u64 {
    if now_ns <= epoch_ns {
        return 0;
    }
    let d = u128::from(now_ns - epoch_ns);
    let num = u128::from(rate.num);
    let den_ns = u128::from(rate.den) * 1_000_000_000;
    // smallest k with k·den_ns/num ≥ d  ⇔  k ≥ d·num/den_ns
    let mut k = (d * num).div_ceil(den_ns) as u64;
    while k > 0 && t_k(epoch_ns, k - 1, rate) >= now_ns {
        k -= 1;
    }
    while t_k(epoch_ns, k, rate) < now_ns {
        k += 1;
    }
    k
}

#[cfg(target_os = "macos")]
mod os {
    #[repr(C)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }
    unsafe extern "C" {
        fn mach_absolute_time() -> u64;
        fn mach_timebase_info(info: *mut Timebase) -> i32;
        fn mach_wait_until(deadline: u64) -> i32;
        fn pthread_set_qos_class_self_np(qos: u32, relpri: i32) -> i32;
    }
    fn timebase() -> (u64, u64) {
        static TB: std::sync::OnceLock<(u64, u64)> = std::sync::OnceLock::new();
        *TB.get_or_init(|| {
            let mut tb = Timebase { numer: 0, denom: 0 };
            // SAFETY: valid out-parameter.
            unsafe { mach_timebase_info(&mut tb) };
            (u64::from(tb.numer.max(1)), u64::from(tb.denom.max(1)))
        })
    }
    pub fn mono_ns() -> u64 {
        let (n, d) = timebase();
        // SAFETY: no preconditions.
        let t = unsafe { mach_absolute_time() };
        (u128::from(t) * u128::from(n) / u128::from(d)) as u64
    }
    pub fn sleep_until_ns(deadline_ns: u64) {
        let (n, d) = timebase();
        let ticks = (u128::from(deadline_ns) * u128::from(d) / u128::from(n)) as u64;
        // SAFETY: no preconditions; returns early only on interruption.
        unsafe { mach_wait_until(ticks) };
    }
    pub fn elevate() -> bool {
        const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;
        // SAFETY: affects only the calling thread.
        unsafe { pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0) == 0 }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod os {
    fn now(clock: libc::clockid_t) -> u64 {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: valid out-parameter.
        unsafe { libc::clock_gettime(clock, &mut ts) };
        ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
    }
    pub fn mono_ns() -> u64 {
        now(libc::CLOCK_MONOTONIC)
    }
    pub fn sleep_until_ns(deadline_ns: u64) {
        let ts = libc::timespec {
            tv_sec: (deadline_ns / 1_000_000_000) as libc::time_t,
            tv_nsec: (deadline_ns % 1_000_000_000) as libc::c_long,
        };
        // SAFETY: valid timespec; absolute deadline on CLOCK_MONOTONIC.
        unsafe {
            libc::clock_nanosleep(
                libc::CLOCK_MONOTONIC,
                libc::TIMER_ABSTIME,
                &ts,
                std::ptr::null_mut(),
            )
        };
    }
    pub fn elevate() -> bool {
        // SCHED_FIFO only if permitted, else a nicer nice (PLAN §5.1).
        let p = libc::sched_param { sched_priority: 10 };
        // SAFETY: affects only the calling thread.
        unsafe {
            libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_FIFO, &p) == 0
                || libc::setpriority(libc::PRIO_PROCESS, 0, -10) == 0
        }
    }
}

#[cfg(windows)]
mod os {
    use windows_sys::Win32::System::Performance::{
        QueryPerformanceCounter, QueryPerformanceFrequency,
    };
    use windows_sys::Win32::System::Threading::{
        CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, GetCurrentThread, INFINITE,
        SetThreadPriority, SetWaitableTimer, THREAD_PRIORITY_TIME_CRITICAL, TIMER_ALL_ACCESS,
        WaitForSingleObject,
    };
    fn freq() -> u64 {
        static F: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
        *F.get_or_init(|| {
            let mut f = 0i64;
            // SAFETY: valid out-parameter.
            unsafe { QueryPerformanceFrequency(&mut f) };
            f.max(1) as u64
        })
    }
    pub fn mono_ns() -> u64 {
        let mut c = 0i64;
        // SAFETY: valid out-parameter.
        unsafe { QueryPerformanceCounter(&mut c) };
        (u128::from(c as u64) * 1_000_000_000 / u128::from(freq())) as u64
    }
    pub fn sleep_until_ns(deadline_ns: u64) {
        let now = mono_ns();
        if deadline_ns <= now {
            return;
        }
        // Relative due time in 100 ns units (negative = relative).
        let due = -(((deadline_ns - now) / 100) as i64).max(1);
        thread_local! {
            static TIMER: windows_sys::Win32::Foundation::HANDLE = unsafe {
                CreateWaitableTimerExW(
                    std::ptr::null(),
                    std::ptr::null(),
                    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                    TIMER_ALL_ACCESS,
                )
            };
        }
        TIMER.with(|&t| {
            if t.is_null() {
                std::thread::sleep(std::time::Duration::from_nanos(deadline_ns - now));
                return;
            }
            // SAFETY: valid timer handle owned by this thread.
            unsafe {
                SetWaitableTimer(t, &due, 0, None, std::ptr::null(), 0);
                WaitForSingleObject(t, INFINITE);
            }
        });
    }
    pub fn elevate() -> bool {
        // SAFETY: affects only the calling thread.
        unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) != 0 }
    }
}

/// Machine-wide monotonic time in ns.
pub fn mono_ns() -> u64 {
    os::mono_ns()
}

/// Raise the calling thread to the real-time class PLAN §5.1 names. `false` if refused.
pub fn elevate_current_thread() -> bool {
    os::elevate()
}

/// Spin margin after the OS timer (PLAN §5.1: deadline − 0.5 ms, then spin).
pub const SPIN_NS: u64 = 500_000;

/// Wait until `deadline_ns` on `mono_ns()`: precise OS timer to `deadline − 0.5 ms`, then spin.
/// Returns the lateness in ns (0 when on time).
pub fn wait_until(deadline_ns: u64) -> u64 {
    let now = mono_ns();
    if deadline_ns > now + SPIN_NS {
        os::sleep_until_ns(deadline_ns - SPIN_NS);
    }
    loop {
        let now = mono_ns();
        if now >= deadline_ns {
            return now - deadline_ns;
        }
        std::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_k_is_exact_and_never_accumulates() {
        let r = Rate::P5994;
        assert_eq!(t_k(0, 0, r), 0);
        assert_eq!(t_k(0, 1, r), 16_683_333);
        assert_eq!(t_k(0, 60_000, r), 1_001_000_000_000);
        // 213+ days at 59.94 still exact (u128 inside).
        let k = 1_200_000_000u64;
        assert_eq!(
            t_k(5, k, r),
            5 + (k as u128 * 1001 * 1_000_000_000 / 60000) as u64
        );
        assert_eq!(tc_k(100, 60_000, r), 100 + 10_010_000_000);
        assert_eq!(Rate::P5994.ceil_fps(), 60);
        assert_eq!(Rate::P2997.ceil_fps(), 30);
        assert_eq!(Rate::P50.period_ns(), 20_000_000);
    }

    #[test]
    fn k_at_or_after_brackets_now() {
        for r in [Rate::P5994, Rate::P50, Rate::P2997, Rate::P25] {
            for now in [
                0u64,
                1,
                16_683_333,
                16_683_334,
                999_999_999_999,
                7_777_777_777,
            ] {
                let k = k_at_or_after(1000, 1000 + now, r);
                assert!(t_k(1000, k, r) >= 1000 + now);
                assert!(k == 0 || t_k(1000, k - 1, r) < 1000 + now);
            }
        }
    }

    #[test]
    fn mono_is_monotonic_and_wait_until_reaches_the_deadline() {
        let a = mono_ns();
        let late = wait_until(a + 2_000_000);
        let b = mono_ns();
        assert!(b >= a + 2_000_000);
        // Generous: CI hosts and coalescing gateways oversleep; only "not early" is a contract.
        assert!(late < 200_000_000, "late {late}");
    }
}
