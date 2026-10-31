//! Raw terminal access: mode switching, size, input, and signal cleanup.

use std::io::{self, Write};
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicI32, Ordering};

use crate::app::App;

const ENTER: &str = "\x1b[?1049h\x1b[?25l\x1b[?7l\x1b[?1000h\x1b[?1006h\x1b[2J";
const LEAVE: &str = "\x1b[?1006l\x1b[?1000l\x1b[?7h\x1b[?25h\x1b[0m\x1b[?1049l";

static SIGNAL: AtomicI32 = AtomicI32::new(0);

extern "C" fn on_signal(signal: libc::c_int) {
    SIGNAL.store(signal, Ordering::SeqCst);
}

/// The terminating signal received so far, if any.
pub fn pending_signal() -> Option<i32> {
    match SIGNAL.load(Ordering::SeqCst) {
        0 => None,
        signal => Some(signal),
    }
}

pub fn is_interactive() -> bool {
    // SAFETY: isatty only inspects the file descriptor.
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 && libc::isatty(libc::STDOUT_FILENO) == 1 }
}

pub fn write_out(text: &str) {
    let mut out = io::stdout().lock();
    // A closed terminal means the game is about to exit anyway.
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

/// (columns, rows), falling back to 80x24 if the terminal doesn't report a size.
pub fn size() -> (usize, usize) {
    let mut size = MaybeUninit::<libc::winsize>::zeroed();
    // SAFETY: TIOCGWINSZ writes a winsize through the pointer, which is valid for it.
    let ok = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, size.as_mut_ptr()) } == 0;
    // SAFETY: A zeroed winsize is valid, and ioctl leaves it initialized on success.
    let size = unsafe { size.assume_init() };
    if ok && size.ws_col > 0 && size.ws_row > 0 {
        (size.ws_col as usize, size.ws_row as usize)
    } else {
        (80, 24)
    }
}

/// Waits for input. This uses select because poll is unreliable on macOS terminals.
pub fn input_ready(timeout_ms: u32) -> bool {
    let mut timeout = libc::timeval {
        tv_sec: (timeout_ms / 1000) as libc::time_t,
        tv_usec: ((timeout_ms % 1000) * 1000) as libc::suseconds_t,
    };
    // SAFETY: The fd_set is initialized by FD_ZERO before use, stdin is below
    // FD_SETSIZE, and all pointers outlive the calls.
    unsafe {
        let mut fds = MaybeUninit::<libc::fd_set>::uninit();
        libc::FD_ZERO(fds.as_mut_ptr());
        libc::FD_SET(libc::STDIN_FILENO, fds.as_mut_ptr());
        libc::select(
            libc::STDIN_FILENO + 1,
            fds.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut timeout,
        ) > 0
    }
}

/// Reads available input. Returns Ok(0) when the terminal has closed.
pub fn read_input(buffer: &mut [u8]) -> io::Result<usize> {
    // SAFETY: The pointer and length describe a valid, writable buffer.
    let count = unsafe {
        libc::read(libc::STDIN_FILENO, buffer.as_mut_ptr().cast(), buffer.len())
    };
    if count < 0 { Err(io::Error::last_os_error()) } else { Ok(count as usize) }
}

/// Puts the terminal in game mode, and restores it when dropped, including on panic.
struct RawTerminal {
    saved: libc::termios,
}

impl RawTerminal {
    fn enter() -> io::Result<Self> {
        let mut saved = MaybeUninit::<libc::termios>::uninit();
        // SAFETY: tcgetattr initializes the termios on success, which is checked
        // before assume_init.
        let saved = unsafe {
            if libc::tcgetattr(libc::STDIN_FILENO, saved.as_mut_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }
            saved.assume_init()
        };
        let mut mode = saved;
        // No echo or line buffering. ISIG stays off so Ctrl-C arrives as a quit key
        // and Ctrl-Z can't suspend the game with the terminal still in this mode.
        mode.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG);
        mode.c_cc[libc::VMIN] = 1;
        mode.c_cc[libc::VTIME] = 0;
        // SAFETY: mode is a fully initialized termios.
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSADRAIN, &mode) } != 0 {
            return Err(io::Error::last_os_error());
        }
        write_out(ENTER);
        Ok(RawTerminal { saved })
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        write_out(LEAVE);
        // SAFETY: saved came from tcgetattr.
        unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSADRAIN, &self.saved) };
    }
}

/// Plays until the player quits. Returns the signal number if one ended the game.
pub fn run(app: &mut App) -> io::Result<Option<i32>> {
    let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    // SAFETY: The handler only stores to an atomic, which is async-signal-safe.
    unsafe {
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGHUP, handler);
    }
    let _terminal = RawTerminal::enter()?;
    app.run();
    Ok(pending_signal())
}
