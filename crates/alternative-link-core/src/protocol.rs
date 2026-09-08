use std::net::Ipv4Addr;

#[derive(PartialEq, Debug)]
pub enum Message {
    Discover(Code, Ipv4Addr),
    Ping(Ipv4Addr),
    Ack(Ipv4Addr),
}

impl Message {
    pub fn sender_ip(&self) -> Ipv4Addr {
        match self {
            Message::Discover(_, ip) | Message::Ping(ip) | Message::Ack(ip) => *ip,
        }
    }
}


impl TryFrom<&str> for Message {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {


        match value.trim().split(' ').collect::<Vec<_>>().as_slice() {
            [cmd, ip] if *cmd == "PING" => {
                match ip.parse::<Ipv4Addr>() {
                    Ok(d) => {Ok(Message::Ping(d))},
                    Err(_) => {Err("Parsing str to Ipv4Addr")}
                }
            },
            [cmd, ip] if *cmd == "ACK" => {
                match ip.parse::<Ipv4Addr>() {
                    Ok(d) => {Ok(Message::Ack(d))},
                    Err(_) => {Err("Parsing str to Ipv4Addr")}
                }
            },
            [cmd, ip] if let Ok(g) = ip.parse::<Ipv4Addr>() => {

                if let Ok(code) = Code::try_from(*cmd) {
                    Ok(Message::Discover(code, g))
                } else {
                    Err("Creating the code didn't work")
                }
                
            },
            _ => {Err("Conversion didn't work")}
        }
    }
    
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_to_string() {
        assert_eq!(Code { code_uint: 1337 }.to_string() , "1-3-3-7")
    }

    #[test]
    fn test_code_try_from() {
        assert!(Code::try_from("1-3-3-7").is_ok());
        assert!(Code::try_from("foo-bar").is_err())
    }

    #[test]
    fn test_message_try_from_ping() {
        assert_eq!(Message::try_from("PING 127.0.0.1"), Ok(Message::Ping(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(Message::try_from("PING 127.0.0.").is_err());
        assert!(Message::try_from("PONG 127.0.0.1").is_err());
    }

    #[test]
    fn test_message_try_from_ack() {
        assert_eq!(Message::try_from("ACK 127.0.0.1"), Ok(Message::Ack(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(Message::try_from("ACK 127.0.0.").is_err());
        assert!(Message::try_from("A 127.0.0.1").is_err());
    }

    #[test]
    fn test_message_try_from_discover() {
        assert_eq!(Message::try_from("1-3-3-7 127.0.0.1"), Ok(Message::Discover(Code {code_uint:1337}, Ipv4Addr::new(127, 0, 0, 1))));
        assert!(Message::try_from("1-3-3-7 127.0.0.1").is_ok());
        assert!(Message::try_from("1-3-3-b 127.0.0.1").is_err());
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub struct Code {
    pub code_uint: usize
}

impl TryFrom<&str> for Code {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let value = value.replace("-", "");
        if let Ok(result) = value.parse::<usize>() {
            Ok(Code { code_uint: result })
        } else {
            Err("Parsing didn't work")
        }
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.code_uint.to_string();
        write!(f, "{}", s.chars().collect::<Vec<_>>().chunks(1)
            .map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-"))
    }
}