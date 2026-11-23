//! The `msteams:` URL scheme: which links belong to the app, and the per-user OS
//! registration that hands them to this executable.
//!
//! macOS gets the scheme from the bundle (`CFBundleURLTypes` in `scripts/build-macos.sh`)
//! plus the runtime `register_url_scheme` call, Linux from the desktop entry's
//! `MimeType`, and Windows needs the registry values written here at startup.

use std::io;

/// The scheme Teams puts in shared links (`msteams://…`), matched case-insensitively.
const SCHEME_PREFIX: &str = "msteams:";
/// The text `RemoteCommand::Join` encodes onto the single-instance pipe
/// (`crates/app-state/src/call/remote_command.rs`); this crate sits below app-state,
/// so the prefix is repeated here. Keep the two in step.
const JOIN_COMMAND: &str = "join ";

/// The link, trimmed, when `url` is an `msteams:` URL. Teams can emit `MSTeams://` too.
pub fn msteams_link(url: &str) -> Option<&str> {
    let url = url.trim();
    url.get(..SCHEME_PREFIX.len())
        .filter(|prefix| prefix.eq_ignore_ascii_case(SCHEME_PREFIX))
        .map(|_| url)
}

/// The remote command a second instance forwards for an incoming URL, `None` for
/// any other link.
pub fn msteams_join_command(url: &str) -> Option<String> {
    msteams_link(url).map(|link| format!("{JOIN_COMMAND}{link}"))
}

/// Registers this executable as the current user's `msteams:` handler.
///
/// Only Windows needs an in-app registration; a failure is returned for the caller
/// to log and startup continues.
pub fn register() -> io::Result<()> {
    #[cfg(windows)]
    {
        windows::register()
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

#[cfg(windows)]
mod windows {
    use std::io;
    use std::mem::size_of;

    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegGetValueW, RegSetKeyValueW,
    };
    use windows::core::PCWSTR;

    /// `HKEY_CURRENT_USER\Software\Classes\msteams` and its values; the class name is
    /// the scheme, so the shell routes `msteams:` links to the command below.
    const CLASS_KEY: &str = "Software\\Classes\\msteams";
    const ICON_KEY: &str = "Software\\Classes\\msteams\\DefaultIcon";
    const COMMAND_KEY: &str = "Software\\Classes\\msteams\\shell\\open\\command";
    const DESCRIPTION: &str = "URL:Microsoft Teams Protocol";
    const URL_PROTOCOL: &str = "URL Protocol";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn name_ptr(name: Option<&Vec<u16>>) -> PCWSTR {
        name.map_or(PCWSTR::null(), |name| PCWSTR(name.as_ptr()))
    }

    /// The current `REG_SZ` text of a value, `None` when it is missing or another type.
    fn read_value(key: &str, name: Option<&str>) -> Option<String> {
        let (key, name) = (wide(key), name.map(wide));
        let name = name_ptr(name.as_ref());
        let mut bytes = 0u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                name,
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut bytes),
            )
        };
        if status.is_err() || bytes < size_of::<u16>() as u32 {
            return None;
        }
        let mut buffer = vec![0u16; (bytes as usize).div_ceil(size_of::<u16>())];
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                name,
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        };
        if status.is_err() {
            return None;
        }
        let length = buffer
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..length]))
    }

    fn write_value(key: &str, name: Option<&str>, data: &str) -> io::Result<()> {
        let (key, name, data) = (wide(key), name.map(wide), wide(data));
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                name_ptr(name.as_ref()),
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * size_of::<u16>()) as u32,
            )
        }
        .ok()
        .map_err(|error| io::Error::other(error.message()))
    }

    /// Writes the class values, skipping the ones already in place so a start does not
    /// churn the registry; a moved executable updates the icon and command.
    pub(super) fn register() -> io::Result<()> {
        let executable = std::env::current_exe()?;
        let executable = executable.display();
        let values = [
            (CLASS_KEY, None, DESCRIPTION.to_owned()),
            (CLASS_KEY, Some(URL_PROTOCOL), String::new()),
            (ICON_KEY, None, format!("\"{executable}\",0")),
            (COMMAND_KEY, None, format!("\"{executable}\" \"%1\"")),
        ];
        for (key, name, data) in values {
            if read_value(key, name).as_deref() != Some(data.as_str()) {
                write_value(key, name, &data)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{msteams_join_command, msteams_link};

    #[test]
    fn only_msteams_links_are_handled_and_the_scheme_is_case_insensitive() {
        let link = "msteams://teams.microsoft.com/l/meetup-join/19%3ameeting";
        assert_eq!(msteams_link(link), Some(link));
        assert_eq!(
            msteams_link("  MSTeams://teams.microsoft.com/l/meetup-join/x  "),
            Some("MSTeams://teams.microsoft.com/l/meetup-join/x")
        );
        assert_eq!(
            msteams_link("https://teams.microsoft.com/l/meetup-join/x"),
            None
        );
        assert_eq!(msteams_link("msteams"), None);
        assert_eq!(msteams_link(""), None);
    }

    #[test]
    fn the_forwarded_command_uses_the_existing_join_encoding() {
        assert_eq!(
            msteams_join_command(" msteams://example/meet "),
            Some("join msteams://example/meet".to_owned())
        );
        assert_eq!(msteams_join_command("https://example/meet"), None);
    }
}
