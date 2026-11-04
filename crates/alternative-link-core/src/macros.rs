#[macro_export]
macro_rules! confirm_direct_connection {
    ($ip_receiver:expr, $shared_state:expr, $args:expr) => {
        let ip = if let Some(ipa) = *$ip_receiver.borrow() { ipa } else { continue; };
        report_status(
            &$args,
            "Connection working",
            &[("state", "direct connection confirmed"), ("peer_ip", &ip.to_string())],
        );
        $shared_state.cancel.cancel();
        break;
    };
}

#[macro_export]
macro_rules! status {
    ($args:expr, $fmt:literal $(, $fmt_arg:expr)* ; $($key:ident = $val:expr),+ $(,)?) => {
        report_status(
            &$args,
            &format!($fmt $(, $fmt_arg)*),
            &[$((stringify!($key), &$val.to_string())),+],
        )
    };
}