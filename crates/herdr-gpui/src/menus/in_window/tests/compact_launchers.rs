use super::*;
use crate::{chat::Program, config::ChatLauncher, window::MockPeer};
use herdr_chat_protocol::Agent;

#[gpui::test]
fn installed_agent_buttons_leave_compact_menu_and_controls_reachable(cx: &mut TestAppContext) {
    let peer = MockPeer::advertising(&["tab.create"]);
    let (view, cx) = window(cx);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    view.update(cx, |view, cx| {
        crate::worktree_scripts::tests::connect(view, &peer, cx);
        view.config.show_menu_bar = false;
        view.config.show_window_controls = true;
        view.config.features.acp_chats = true;
        view.sidebar_visible = false;
        view.endpoints[0].connection.target = herdr_client::ConnectTarget::Local;
        view.live.local_daemon_peer = true;
        view.endpoints[0].live = view.live.clone();
        view.chats.program = Program::Found("/opt/herdr-acp".into());
        let id = view.endpoints[0].id.clone();
        view.chats.agents.record(&id, Agent::ALL.to_vec());
        cx.notify();
    });
    for placement in [
        ChatLauncher::Beside,
        ChatLauncher::Leading,
        ChatLauncher::Trailing,
        ChatLauncher::Button,
    ] {
        for font_size in [8., 12.] {
            view.update(cx, |view, cx| {
                view.config.chat.launcher = placement;
                view.config.ui.size = font_size;
                view.chats.launcher_open = Some(view.group_slots()[0].id);
                view.chats.last_started = Agent::ALL
                    .iter()
                    .copied()
                    .max_by_key(|agent| agent.display_name().len());
                cx.notify();
            });
            for width in [480., 560., 800.] {
                cx.simulate_resize(size(px(width), px(600.)));
                draw(cx);
                if placement == ChatLauncher::Button {
                    let width = view.read_with(cx, |view, _| {
                        px((Agent::ALL.len() - 1) as f32 * view.tab_layout().new_tab_width())
                    });
                    assert_eq!(
                        cx.debug_bounds("chat-launcher-slide").unwrap().size.width,
                        width
                    );
                }
                let menu = cx.debug_bounds("application-menu-launcher").unwrap();
                let close = cx.debug_bounds("window-close").unwrap();
                assert!(
                    menu.left() >= px(0.)
                        && menu.right() <= close.left()
                        && close.right() <= px(width),
                    "{placement:?}, font {font_size}, width {width}: menu {menu:?}, close {close:?}"
                );
            }
        }
    }
}
