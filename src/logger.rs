/// 初始化日志系统
pub fn init() {
    // 设置日志级别为 off 来关闭日志输出
    let env = env_logger::Env::default()
        .filter_or("LOG_LEVEL", "debug")
        .write_style_or("LOG_STYLE", "always");
    env_logger::init_from_env(env);
}
