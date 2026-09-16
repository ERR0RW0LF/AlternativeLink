use std::io::{Write, stdin, stdout};


pub fn yes_no_prompt(question_text: &str, default: Option<bool>) -> Option<bool>{
    
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
        "y" | "yes" => {
            Some(true)
        },
        "n" | "no" => {
            Some(false)
        },
        _ if default == Some(true) => {
            Some(true)
        },
        _ if default == Some(false) => {
            Some(false)
        },
        _ => {
            None
        }
    }
}

pub fn _list_prompt(question_text: &str, answers: Vec<&str>, default: Option<usize>) -> Option<usize> {
    for (i, text) in answers.iter().enumerate() {
        println!("- {i:0>3}: {}", text);
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
        Ok(i) => {Some(i)},
        Err(_e) => {
            if let Some(i) = default {
                return Some(i);
            }
            None
        }
    }
} 