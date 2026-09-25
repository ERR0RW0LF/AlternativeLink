use std::io::{Write, stdin, stdout};

pub fn yes_no_prompt(question_text: &str, default: Option<bool>) -> Option<bool> {
    
    println!("{}", question_text);

    println!();

    match default {
        Some(t) => {
            if t {
                print!("Y/n: ");
            } else {
                print!("y/N: ");
            }
        },
        None => {
            print!("y/n: ");
        }
    }
    let mut buffer = String::new();
    let stdin = stdin();
    if let Err(e) = stdout().flush() {
        eprintln!("{}", e);
    }

    if let Err(e) = stdin.read_line(&mut buffer) {eprintln!("{}", e); return None};
    match buffer.trim().to_lowercase().as_str() {
        "y" | "yes" => Some(true),
        "n" | "no" => Some(false),
        _ => default,
    }
}

pub fn list_prompt(question_text: &str, answers: Vec<&str>, default: Option<usize>) -> Option<usize> {
    for (i, text) in answers.iter().enumerate() {
        println!("- {i:02}: {}", text);
    }
    println!("{}", question_text);
    print!("Number of your choice: ");
    let mut buffer = String::new();
    let stdin = stdin();
    if let Err(e) = stdout().flush() {
        eprintln!("{}", e);
    }
    if let Err(e) = stdin.read_line(&mut buffer) {eprintln!("{}", e); return None};
    match buffer.trim().parse::<usize>() {
        Ok(i) => Some(i),
        Err(_e) => default
    }
}

pub fn number_prompt(question_text: &str, default: Option<isize>) -> Option<isize> {
    print!("{}", question_text);
    if let Some(i) = default {
        println!(" (default: {})", i);
    } else {
        println!();
    }

    print!("Input a number: ");
    let mut buffer = String::new();
    let stdin = stdin();
    if let Err(e) = stdout().flush() {
        eprintln!("{}", e);
    }
    if let Err(e) = stdin.read_line(&mut buffer) {eprintln!("{}", e); return None};

    match buffer.trim().parse::<isize>() {
        Ok(i) => Some(i),
        Err(_e) => default
    }
}