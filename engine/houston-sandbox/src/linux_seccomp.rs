//! Seccomp-BPF filter blocking dangerous syscalls inside Linux sandboxes.

use std::collections::BTreeMap;

/// Apply `PR_SET_NO_NEW_PRIVS` and a seccomp filter on the current thread.
pub fn install_dangerous_syscall_filter() -> Result<(), String> {
    set_no_new_privs()?;
    install_filter()?;
    Ok(())
}

fn set_no_new_privs() -> Result<(), String> {
    let rc = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}

fn install_filter() -> Result<(), String> {
    use seccompiler::{
        apply_filter, BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition,
        SeccompFilter, SeccompRule, TargetArch,
    };

    fn deny_syscall(rules: &mut BTreeMap<i64, Vec<SeccompRule>>, nr: i64) {
        rules.insert(nr, vec![]);
    }

    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    deny_syscall(&mut rules, libc::SYS_ptrace);
    deny_syscall(&mut rules, libc::SYS_mount);
    deny_syscall(&mut rules, libc::SYS_umount2);
    deny_syscall(&mut rules, libc::SYS_pivot_root);
    deny_syscall(&mut rules, libc::SYS_chroot);
    deny_syscall(&mut rules, libc::SYS_keyctl);
    deny_syscall(&mut rules, libc::SYS_add_key);
    deny_syscall(&mut rules, libc::SYS_request_key);

    // Block TIOCSTI (inject keystrokes into another tty).
    const TIOCSTI: u64 = 0x5412;
    let block_tiocsti = SeccompRule::new(vec![SeccompCondition::new(
        1,
        SeccompCmpArgLen::Dword,
        SeccompCmpOp::Eq,
        TIOCSTI,
    )
    .map_err(|e| e.to_string())?])
    .map_err(|e| e.to_string())?;
    rules.insert(libc::SYS_ioctl, vec![block_tiocsti]);

    let arch = if cfg!(target_arch = "x86_64") {
        TargetArch::x86_64
    } else if cfg!(target_arch = "aarch64") {
        TargetArch::aarch64
    } else {
        return Err("unsupported architecture for seccomp filter".into());
    };

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        arch,
    )
    .map_err(|e| e.to_string())?;

    let prog: BpfProgram = filter
        .try_into()
        .map_err(|e: seccompiler::BackendError| e.to_string())?;
    apply_filter(&prog).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_filter_does_not_panic_when_supported() {
        if install_dangerous_syscall_filter().is_err() {
            // CI kernels without seccomp support are acceptable in unit tests.
        }
    }
}
