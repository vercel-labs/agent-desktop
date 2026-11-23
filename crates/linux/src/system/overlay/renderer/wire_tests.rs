use super::*;

#[test]
fn a_line_round_trips() {
    let mut buffer = Vec::new();
    write_line(&mut buffer, &request("update", &json!({"op": "conceal"}))).expect("written");
    assert_eq!(buffer.last(), Some(&b'\n'));
    let read = read_line(buffer.as_slice()).expect("read");
    assert_eq!(read, json!({"method": "update", "body": {"op": "conceal"}}));
}

#[test]
fn a_line_without_its_newline_is_refused() {
    assert_eq!(
        read_line(&b"{}"[..]).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn a_line_past_the_limit_is_refused_without_reading_it_all() {
    let long = vec![b' '; MAX_LINE_BYTES as usize + 10];
    assert_eq!(
        read_line(long.as_slice()).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}
