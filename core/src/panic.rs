use std::panic::PanicInfo;

use crate::e;

fn get_payload<'a>(panic_info: &'a PanicInfo<'a>) -> Option<&'a str> {
    panic_info
        .payload()
        .downcast_ref::<String>() // 尝试将 payload 转型为 String 或者 &str
        .map(|e| e.as_str())
        .or(panic_info
            .payload()
            .downcast_ref::<&str>()
            .map(|e| e.as_ref()))
}

/// 自定义 panic 行为
pub fn custom_panic() {
    std::panic::set_hook(Box::new(|panic_info| {
        if let (Some(payload), Some(location)) = (get_payload(panic_info), panic_info.location()) {
            println!(
                "{}:{} -> Panic occurred: {payload}",
                location.file(),
                location.line(),
            );
        } else {
            e!("Panic occurred: {panic_info}");
        }
    }));
}
