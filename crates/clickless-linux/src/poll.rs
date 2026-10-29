use libc::{POLLERR, POLLHUP, POLLIN, POLLNVAL, c_int, poll, pollfd};

#[derive(Default)]
pub struct Readiness {
    pub ready: Vec<c_int>,
    pub removed: Vec<c_int>,
}

pub fn poll_devices(fds: &[c_int], timeout_ms: i32) -> std::io::Result<Readiness> {
    let mut pollfds: Vec<pollfd> = fds
        .iter()
        .map(|&fd| pollfd {
            fd,
            events: POLLIN,
            revents: 0,
        })
        .collect();

    let res = unsafe {
        poll(
            pollfds.as_mut_ptr(),
            pollfds.len() as libc::nfds_t,
            timeout_ms,
        )
    };
    if res < 0 {
        let error = std::io::Error::last_os_error();
        return if error.kind() == std::io::ErrorKind::Interrupted {
            Ok(Readiness::default())
        } else {
            Err(error)
        };
    }

    let mut readiness = Readiness::default();
    for pfd in pollfds {
        if pfd.revents & (POLLERR | POLLHUP | POLLNVAL) != 0 {
            readiness.removed.push(pfd.fd);
        } else if pfd.revents & POLLIN != 0 {
            readiness.ready.push(pfd.fd);
        }
    }
    Ok(readiness)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

    #[test]
    fn hangup_is_removed_instead_of_ignored() {
        let (reader, writer) = UnixStream::pair().unwrap();
        drop(writer);
        let state = poll_devices(&[reader.as_raw_fd()], 0).unwrap();
        assert_eq!(state.removed, vec![reader.as_raw_fd()]);
        assert!(state.ready.is_empty());
    }

    #[test]
    fn readable_source_is_ready_and_invalid_source_is_removed() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(b"x").unwrap();
        let state = poll_devices(&[reader.as_raw_fd(), c_int::MAX], 0).unwrap();
        assert_eq!(state.ready, vec![reader.as_raw_fd()]);
        assert_eq!(state.removed, vec![c_int::MAX]);
    }
}
