use chrono::Local;


pub fn setup() -> Result<(), fern::InitError>{
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{:<5}] {}",
                now(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Trace)
        .chain(std::io::stdout())
        .chain(fern::log_file("C:\\Users\\Administrator\\Documents\\ajemi.log")?)
        .apply()?;
    Ok(())   
}

fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

