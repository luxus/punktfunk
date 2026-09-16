//! The streamed head's window list, and the three verbs that act on one.
//!
//! A phone in a full-screen stream cannot see what is behind the game. This is
//! the list that answers that, per compositor, in one shape.
//!
//! Scope is the streamed output, never the whole desk: a session already sees
//! that head's pixels, so its titles are not a new disclosure, and the
//! operator's other monitors stay out of the payload. The host gates the
//! verbs on the session's grants ([`WindowVerb::required_grants`]).

// Only the backend arms need the crate prelude; the types are plain serde.
#[cfg(target_os = "linux")]
use super::*;

/// One compositor toplevel, as every backend reports it.
///
/// `id` is the backend's own handle (a Hyprland address, a sway con id) and is
/// opaque above this module: it goes back to the backend that minted it and
/// nowhere else. Defined on every platform — the management route and its
/// schema exist on a Windows host too, which simply lists nothing.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct Toplevel {
    /// Backend handle. Never parsed above the backend, never reused across one.
    pub id: String,
    pub title: String,
    /// Wayland `app_id`, or the X11 class on an Xwayland window.
    pub app_id: String,
    /// `None` where the compositor does not report one (some Xwayland cons).
    #[schema(value_type = u32, required = false)]
    pub pid: Option<u32>,
    pub focused: bool,
    pub fullscreen: bool,
    /// Workspace name as the compositor spells it, not its id.
    pub workspace: String,
    /// The head this window is on — always the streamed one, by construction.
    pub output: String,
}

/// What a client may ask of one window.
///
/// Defined on every platform: the management route decodes it before any
/// backend is consulted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WindowVerb {
    /// Raise and give it focus.
    Focus,
    /// Make it full-screen on its head.
    Fullscreen,
    /// Ask it to close. Destructive, and the only one that can lose work.
    Close,
}

impl WindowVerb {
    /// Grant bits a session needs to ask for this verb, ANDed against its live
    /// mask. Exhaustive, no wildcard: a new verb is a compile error until it is
    /// priced. Listing is free — a session already sees the head it streams.
    pub fn required_grants(self) -> u32 {
        use punktfunk_core::quic::{GRANT_GAMEPAD, GRANT_KEYBOARD, GRANT_LAUNCH, GRANT_POINTER};
        match self {
            // A device that can send input at all can raise a window by
            // clicking it, so Controller only reaches this and View only does not.
            Self::Focus | Self::Fullscreen => GRANT_GAMEPAD | GRANT_POINTER | GRANT_KEYBOARD,
            // The inverse of starting something, and priced like it: Controller
            // only deliberately withholds Launch so the owner drives what runs.
            Self::Close => GRANT_LAUNCH,
        }
    }

    /// Does `mask` carry any of the bits this verb needs?
    pub fn permitted_by(self, mask: u32) -> bool {
        mask & self.required_grants() != 0
    }

    /// Operator-log name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Fullscreen => "fullscreen",
            Self::Close => "close",
        }
    }
}

/// Windows on streamed head `output`, newest compositor state each call.
///
/// Empty on any backend that cannot be asked, and on any read that fails —
/// a window list is never worth failing a request over. `output` is checked
/// against the backend's own mint, as in [`focus_streamed_output`]: a physical
/// connector's windows are the operator's, not this session's.
#[cfg(target_os = "linux")]
pub fn list_toplevels(compositor: Compositor, output: &str) -> Vec<Toplevel> {
    match compositor {
        Compositor::Hyprland if hyprland::is_managed_output(output) => {
            hyprland::toplevels(Some(output))
        }
        Compositor::Wlroots if wlroots::is_managed_output(output) => {
            wlroots::toplevels(Some(output))
        }
        // No `_` arm: a new backend must decide here. KWin drives a privileged
        // Wayland protocol, not a script channel, so its list is new machinery;
        // Mutter exposes no toplevel list; gamescope nests one app; Windows has
        // no compositor to ask.
        Compositor::Hyprland
        | Compositor::Wlroots
        | Compositor::Kwin
        | Compositor::Mutter
        | Compositor::Gamescope
        | Compositor::Windows => {
            tracing::info!(
                compositor = compositor.id(), output = %output,
                "this compositor does not report a window list — the client sees the stream only"
            );
            Vec::new()
        }
    }
}

/// Every window on every head, for the host's own placement decisions.
///
/// Never a client's answer: it names the operator's other monitors, which is
/// what [`list_toplevels`] exists to keep out. The window stage needs it to
/// notice a game that opened on the wrong screen.
#[cfg(target_os = "linux")]
pub fn list_all_toplevels(compositor: Compositor) -> Vec<Toplevel> {
    match compositor {
        Compositor::Hyprland => hyprland::toplevels(None),
        Compositor::Wlroots => wlroots::toplevels(None),
        // No `_` arm; same backends as [`list_toplevels`].
        Compositor::Kwin | Compositor::Mutter | Compositor::Gamescope | Compositor::Windows => {
            Vec::new()
        }
    }
}

/// Carry window `id` onto head `output`.
///
/// Host-side placement, not a client verb: the game opened where the
/// compositor put it, and the player can only see the streamed head.
#[cfg(target_os = "linux")]
pub fn move_toplevel_to_output(
    compositor: Compositor,
    id: &str,
    output: &str,
) -> anyhow::Result<()> {
    match compositor {
        Compositor::Hyprland => hyprland::move_to_output(id, output),
        Compositor::Wlroots => wlroots::move_to_output(id, output),
        // No `_` arm. KWin/Mutter/gamescope/Windows never list, so nothing
        // here has an id to move.
        Compositor::Kwin | Compositor::Mutter | Compositor::Gamescope | Compositor::Windows => {
            anyhow::bail!("{} cannot move a window", compositor.id())
        }
    }
}

/// Act on one window of streamed head `output`.
///
/// `id` must still be in [`list_toplevels`] for this head: an id the
/// compositor recycled, or one on the operator's own monitor, is refused here
/// rather than acted on. The caller has already checked the session's grants.
#[cfg(target_os = "linux")]
pub fn window_action(
    compositor: Compositor,
    output: &str,
    verb: WindowVerb,
    id: &str,
) -> anyhow::Result<()> {
    // Re-read rather than trust a list the client is holding: between its fetch
    // and this call the id may have died, and a stale close hits whatever now
    // answers to it.
    if !list_toplevels(compositor, output)
        .iter()
        .any(|w| w.id == id)
    {
        anyhow::bail!("no window {id} on {output}");
    }
    match compositor {
        Compositor::Hyprland => hyprland::window_action(verb, id),
        Compositor::Wlroots => wlroots::window_action(verb, id),
        // No `_` arm. Unreachable while the backends above are the only ones
        // that list, and a compile error the day another one does.
        Compositor::Kwin | Compositor::Mutter | Compositor::Gamescope | Compositor::Windows => {
            anyhow::bail!("{} cannot act on a window", compositor.id())
        }
    }
}

/// Token that changes when this compositor's windows do, so an idle session
/// costs no `hyprctl`. `None` = this backend cannot say; re-read every time.
#[cfg(target_os = "linux")]
pub fn toplevels_token(compositor: Compositor) -> Option<u64> {
    match compositor {
        Compositor::Hyprland => hyprland::window_gen(),
        // No `_` arm. sway has an IPC subscribe, but the host runs no reader on
        // it; the rest have no list to watch.
        Compositor::Wlroots
        | Compositor::Kwin
        | Compositor::Mutter
        | Compositor::Gamescope
        | Compositor::Windows => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use punktfunk_core::quic::{
        GRANT_PRESET_CONTROLLER_ONLY, GRANT_PRESET_FULL, GRANT_PRESET_VIEW_ONLY,
    };

    /// The three presets against the three verbs. A spectator reads and sends
    /// nothing; a pad guest raises a window but never closes one.
    #[test]
    fn the_presets_price_each_verb_the_way_the_access_levels_read() {
        for verb in [WindowVerb::Focus, WindowVerb::Fullscreen, WindowVerb::Close] {
            assert!(
                !verb.permitted_by(GRANT_PRESET_VIEW_ONLY),
                "view only sends {}",
                verb.as_str()
            );
            assert!(verb.permitted_by(GRANT_PRESET_FULL), "full lacks {verb:?}");
        }
        assert!(WindowVerb::Focus.permitted_by(GRANT_PRESET_CONTROLLER_ONLY));
        assert!(WindowVerb::Fullscreen.permitted_by(GRANT_PRESET_CONTROLLER_ONLY));
        // Closing is the inverse of launching, and a pad guest has no Launch.
        assert!(!WindowVerb::Close.permitted_by(GRANT_PRESET_CONTROLLER_ONLY));
    }

    /// A verb is gated on its own bits, not on "any grant at all": a mic-only
    /// custom mask sends nothing here.
    #[test]
    fn an_unrelated_grant_does_not_buy_a_window_verb() {
        let mic = punktfunk_core::quic::GRANT_MIC;
        assert!(!WindowVerb::Focus.permitted_by(mic));
        assert!(!WindowVerb::Close.permitted_by(mic));
        assert!(WindowVerb::Close.permitted_by(punktfunk_core::quic::GRANT_LAUNCH));
    }
}
