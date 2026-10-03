use libc::{poll, pollfd, POLLIN, c_int};
use std::os::unix::io::AsRawFd;

pub fn poll_devices(fds: &[c_int], timeout_ms: i32) -> std::io::Result<Vec<c_int>> {
    let mut pollfds: Vec<pollfd> = fds.iter().map(|&fd| pollfd {
        fd,
        events: POLLIN,
        revents: 0,
    }).collect();

    let res = unsafe { poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, timeout_ms) };
    if res < 0 {
        return Err(std::io::Error::last_os_error());
    }

    let mut ready = Vec::new();
    for pfd in pollfds {
        if pfd.revents & POLLIN != 0 {
            ready.push(pfd.fd);
        }
    }
    Ok(ready)
}
