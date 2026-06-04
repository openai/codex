use crossterm::Command;
use pretty_assertions::assert_eq;

use super::ClearTabStatus;
use super::SetTabStatus;
use super::TabStatus;

#[test]
fn working_emits_orange_with_matching_text_color() {
    let mut out = String::new();
    SetTabStatus(TabStatus::Working)
        .write_ansi(&mut out)
        .expect("encode tab status");
    assert_eq!(
        out,
        "\x1b]21337;status=Working;indicator=#ff9500;status-color=#ff9500\x07"
    );
}

#[test]
fn waiting_emits_blue_with_matching_text_color() {
    let mut out = String::new();
    SetTabStatus(TabStatus::Waiting)
        .write_ansi(&mut out)
        .expect("encode tab status");
    assert_eq!(
        out,
        "\x1b]21337;status=Waiting;indicator=#5f87ff;status-color=#5f87ff\x07"
    );
}

#[test]
fn idle_uses_dim_text_color() {
    let mut out = String::new();
    SetTabStatus(TabStatus::Idle)
        .write_ansi(&mut out)
        .expect("encode tab status");
    assert_eq!(
        out,
        "\x1b]21337;status=Idle;indicator=#00d75f;status-color=#888888\x07"
    );
}

#[test]
fn clear_emits_empty_fields() {
    let mut out = String::new();
    ClearTabStatus.write_ansi(&mut out).expect("encode clear");
    assert_eq!(out, "\x1b]21337;status=;indicator=;status-color=\x07");
}
