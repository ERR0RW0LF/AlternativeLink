use crate::engine::EngineArgs;

pub fn report_status(args: &EngineArgs, message: &str, json_fields: &[(&str, &str)]) {
    if args.json {
        let fields: Vec<String> = json_fields
            .iter()
            .map(|(k, v)| format!("\"{}\":\"{}\"", k, v))
            .collect();
        println!("{{\"status\":\"{}\",{}}}", message, fields.join(","));
    } else {
        println!("{}", message);
    }
}