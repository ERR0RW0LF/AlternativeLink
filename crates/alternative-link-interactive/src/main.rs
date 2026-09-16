use crate::prompts::yes_no_prompt;

mod prompts;

fn main() {
    println!("Hello, world!");
    let result = yes_no_prompt("Hello World?", None);
    println!("{:#?}",result);
}

