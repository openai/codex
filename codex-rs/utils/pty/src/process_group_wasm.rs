use std::io;

pub fn set_parent_death_signal(_parent_pid: i32) -> io::Result<()> {
    Ok(())
}

pub fn detach_from_tty() -> io::Result<()> {
    Ok(())
}

pub fn set_process_group() -> io::Result<()> {
    Ok(())
}

pub fn kill_process_group_by_pid(_pid: u32) -> io::Result<()> {
    Ok(())
}

pub fn terminate_process_group(_process_group_id: u32) -> io::Result<bool> {
    Ok(false)
}

pub fn kill_process_group(_process_group_id: u32) -> io::Result<()> {
    Ok(())
}

pub fn kill_child_process_group<T>(_child: &mut T) -> io::Result<()> {
    Ok(())
}
