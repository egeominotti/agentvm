//! A VM's console is a terminal's output: its control sequences are removed before a person reads it.

use agentvm::domain::console_text::readable;

#[test]
fn terminal_control_sequences_are_removed() {
    // What a Debian console really prints before its login prompt.
    let raw = "\x1b[6n\x1b[32766;32766H\x1b[6n\x1b[!p\x1b]104\x07\x1b[?7h\x1b[6n\r\r\nDebian GNU/Linux 13 agentvm hvc0\r\n\r\nagentvm login: ";
    assert_eq!(readable(raw), "\nDebian GNU/Linux 13 agentvm hvc0\n\nagentvm login: ");
}

#[test]
fn colors_go_and_the_text_stays() {
    assert_eq!(readable("\x1b[1;32mOK\x1b[0m  Started \x1b]0;title\x1b\\ssh"), "OK  Started ssh");
    assert_eq!(readable("plain text, tabs\tand ünïcode"), "plain text, tabs\tand ünïcode");
}

#[test]
fn a_cut_sequence_at_the_end_does_not_swallow_text_before_it() {
    assert_eq!(readable("booting\x1b[3"), "booting");
}
