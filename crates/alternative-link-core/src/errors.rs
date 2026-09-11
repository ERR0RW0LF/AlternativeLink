use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryError {
    /// Was unable to enumerate the interfaces from the os.
    UnableToEnumInterfaces { os_error: String},

    /// Was unable to read the user input.
    StdInRead { os_error: String},

    /// Unable to cast user input to a valid usize.
    InputParsing { os_error: String},
} 

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiscoveryError::UnableToEnumInterfaces { os_error } => {
                write!(f, "unable to enumerate interfaces: {os_error}")
            },
            DiscoveryError::StdInRead { os_error } => {
                write!(f, "unable to get user input: {os_error}")
            },
            DiscoveryError::InputParsing { os_error } => {
                write!(f, "unable to parse user input to a usize: {os_error}")
            }
        }
    }
}

impl std::error::Error for DiscoveryError {}