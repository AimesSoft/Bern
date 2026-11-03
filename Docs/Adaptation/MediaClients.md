**English** · [简体中文](../zh-CN/Adaptation/MediaClients.md)

# Emby and Jellyfin client adaptation

- Document version: v0.1
- Official sources checked on: 2026-10-09
- Scope: media clients developed with Bern; as a framework validation scenario, server-side business logic is not written into the Bern core.
- Governing constraints: [architecture design](../Design.md) and [general adaptation principles](README.md).

Emby and Jellyfin provide client documentation, APIs, brand assets and some interface rules. This search did not locate a unified design specification, comparable to Apple's HIG, that covers all client platforms. Therefore native interaction follows the guidelines of the corresponding operating system, server connection and media behavior follow each project's official protocol, and the page organization below is a design recommendation for Bern clients.

## Scope of the official materials

| Official material | What it can establish | What cannot be inferred from it |
| --- | --- | --- |
| Emby client user documentation | Home screen configuration, feature behavior, theme and interaction options of specific clients | That all platforms must use the same controls and layout |
| Emby Plugin UI | How the settings pages of server plugins are generated declaratively | A cross-platform HIG for complete third-party players |
| Emby REST and WebSocket documentation | Session control, playback state reporting and capability fields | That a request having been sent means the device has executed it |
| Jellyfin Branding and jellyfin-ux | Project naming, logo usage and shared visual assets | That third-party clients must replicate Jellyfin Web |
| Jellyfin client and media documentation | Sign-in methods, the range of clients, the dimensions for judging playback compatibility | That every native client executes server CSS |
| Jellyfin Style Guides | Source code style | A visual and interaction design system |

Emby's plugin interface is generated from model/property descriptions, and it applies to plugin configuration. Jellyfin's Style Guides explicitly discuss code style; the jellyfin-ux README mainly lists assets such as brand images and logos. They can serve as references but cannot replace platform adaptation guidelines. [Emby Plugin UI](https://dev.emby.media/doc/plugins/ui/index.html), [Jellyfin Style Guides](https://jellyfin.org/docs/general/style-guides/), [Jellyfin UX](https://github.com/jellyfin/jellyfin-ux).

## Emby official behavior and Bern mapping

### Home screen and browsing

Emby's current home screen documentation targets Server 4.10.x and later: sections can configure filters, the displayed images and the scroll direction; the Auto scroll direction is chosen by the client based on size, orientation and device form factor. The older home screen configuration has separate documentation, and the new options must not be sent unconditionally to older servers. [User Preferences Home Screen](https://emby.media/support/articles/apps/common/User-Prefs-HomeScreen.html).

A Bern client keeps the home screen sections, order and user preferences that the server supports, and presents them with its own components. Narrow screens can reduce the number of columns shown at once, TV can use horizontal content rows reachable with the remote, and desktop can use grids with higher information density. Reflow must not change the meaning of filters, the selected media or the playback queue; for unsupported configuration, the reason must be shown clearly.

### Remote sessions and playback state

Emby's Remote Control API first enumerates controllable sessions, then dispatches playback or general controls by session identity; sessions provide capability information such as SupportedCommands. The WebSocket documentation describes notifications and remote control messages separately. [Remote Control](https://dev.emby.media/doc/restapi/Remote-Control.html), [Web Socket](https://dev.emby.media/doc/restapi/Web-Socket.html).

Bern mapping rules:

- The user first explicitly chooses the playback target, and the interface keeps showing the name of the local or remote device; devices with the same name are still distinguished by stable identity.
- The server's Session ID, Bern's session_id and player_id are different namespaces; the service adapter maintains the mapping, and they cannot be treated directly as the same ID.
- A remote playback request first goes through Bern's registered commands and permission checks, and is then sent by the Emby adapter; state reports from the remote end update the observed state of the same playback target.
- `SupportedCommands` affects whether actions are available; it does not dynamically rewrite Bern's compiled command schema.
- Connecting, sent, confirmed, failed and target offline are shown separately. When the protocol cannot confirm execution, the client does not claim "now playing".

Emby Playback Check-ins report start, progress and stop separately, and carry information such as the media source, the playback session and tracks. Its current guide recommends reporting progress about every 10 seconds and reporting promptly after user interaction; this frequency is Emby API behavior and cannot become Bern's drawing clock or a fixed rule for all backends. [Playback Check-ins](https://dev.emby.media/doc/restapi/Playback-Check-ins.html).

Bern reports started/stopped only on the basis of actual playback results. After switching sources, switching the remote target or reconnecting, delayed reports from the old playback session must not overwrite the new session. Server-side progress estimates can be used for smooth display, but the update time and the connection state must be kept.

### Theme and accessibility

The accessibility notes for the Emby Windows client cover keyboard focus and high contrast; there, automatically following the system's high contrast requires that the Windows theme is enabled and that light and dark mode is not overridden. This is a configuration condition of that client, not a reason for Bern to require users to configure the theme manually. [Feature Accessibility](https://emby.media/support/articles/apps/windows/features/Win-Feature-Accessibility.html).

Bern controls still follow the automatic light and dark mode and high contrast contract. Covers and videos returned by the server keep the content's original colors; colors, title length and poster aspect ratio cannot become conditions for determining the kind of device.

## Jellyfin official behavior and Bern mapping

### Brand and client independence

Jellyfin's branding guidelines require third parties to use their own name and logo, and the compatibility relationship can be stated in descriptions. The inclusion criteria of the official client directory and interface design requirements are different questions; this chapter does not present "supports Jellyfin" as "official Jellyfin client". [Branding](https://jellyfin.org/docs/general/contributing/branding/), [Clients](https://jellyfin.org/docs/general/clients/).

Jellyfin Custom CSS only applies to clients that use Jellyfin Web and does not cover all clients. Native Bern frontends therefore do not need to interpret or execute arbitrary server CSS; user display preferences that need to be supported should be mapped to defined, typed settings. [CSS Customization](https://jellyfin.org/docs/general/clients/css-customization/).

### TV sign-in

Quick Connect uses a temporary code with which an already signed-in device authorizes a new client, which suits input-limited devices such as TVs; the server can turn the feature off, and "allowing sign-in" and "authorizing other devices" are different client capabilities. [Quick Connect](https://jellyfin.org/docs/general/server/quick-connect/).

Bern's TV connection page offers this entry point when the server supports it, and keeps clear alternative sign-in paths. Waiting for authorization, authorized, expired, denied and code regenerated are different states. After switching servers or users, an old authorization result must not enter the new session. The code should be easy to read from a distance, and account entry is not designed so that it can only be completed with a phone.

Emby sign-in is implemented separately by the Emby adapter; the historical relationship between the two products is no reason to send Jellyfin Quick Connect requests directly.

### Media compatibility

Jellyfin's official media notes decide between Direct Play, Direct Stream and transcoding based on the combination of container, video, audio and subtitles. Subtitles can change the playback method, so direct playability cannot be asserted by checking only the file extension or a single video codec name. [Codec Support](https://jellyfin.org/docs/general/clients/codec-support/).

Bern's MediaCapabilities come from the real executor, and the service adapter negotiates the playback source based on them. The UI offers clear choices of quality, audio track, subtitles and playback target; debug details can explain why transcoding happens, but the normal playback flow does not expose internal protocol fields. Switching subtitles or quality requires a source version and a request identity, so that multiple asynchronous negotiations do not overwrite each other.

## Bern media client page contract

The table below is Bern's product design recommendation, with features declared by the app's Feature Contract; it is not a page order officially prescribed by Emby or Jellyfin.

| Page or action | Compact touch | Wide window, keyboard and mouse | TV and remote |
| --- | --- | --- | --- |
| Server and user selection | Single-column flow that clearly distinguishes connection errors from sign-in errors | Saved servers and their details can be shown side by side | Large type, little input, pairing sign-in when supported |
| Libraries and home | The current section and a scrollable grid | Side navigation and a content grid | Content rows or grids with clear focus |
| Search and filters | A search page or an overlay panel; filters can be backed out of | Search field, keyboard shortcuts and a secondary filter area | A search entry point reachable with the remote, no reliance on hover |
| Details and episodes | Level-by-level navigation that keeps the position in the source list | List-detail or a secondary panel | Clear play/resume entry points; focus restored to the original item |
| Playback controls | Touch controls and a discoverable audio track and subtitle menu | Keyboard and mouse in parallel; menus list the keyboard shortcuts | Arrow keys, select, back and the play/pause key fully usable |
| Playback target | The current local or remote target shown explicitly | Target list and state can be queried | Targets can still be viewed and switched on the big screen, not just a vague casting icon |
| Settings | Category navigation or a virtual window | A native settings window or an in-app panel | Grouped by task; full-screen virtual window and a clear way back |

Resume and play from the beginning are different commands; switching views must not dispatch playback again. Operations such as restoring progress, favoriting, marking as watched, subtitle selection and remote control all go into the unified engine catalog. The four-entry-point contract cannot be bypassed by calling an external player's private callbacks.

Authentication, the media catalog, playback history, server preferences and the service protocol live in the app's business modules. The Bern core provides only the generic command, state, navigation, Window, Video, theme and accessibility mechanisms.

## State and error design

| Situation | Differences the client must express |
| --- | --- |
| No server found | Discovery failure and an entry point for a manual address; cannot be directly judged as an account error |
| Local network permission denied | The permission state and the server being offline are reported separately; recovery is guided according to platform rules |
| Session expired | Keep the navigation that is allowed to be restored, and re-check access permissions after re-authentication |
| Incompatible media | Distinguish the local backend not supporting it, the server being able to transcode, and no available playback method |
| Remote end dropped | The target being unavailable is shown together with the last observed state; playback success is no longer confirmed |
| Progress known but total duration unknown | Do not show an unknown length as zero, and do not offer arbitrary seeking that cannot be completed |
| User or server switch | Clear subscriptions and sensitive display belonging to the old identity, and do not mix up progress or authorization |

Local network and background playback requirements vary with the target platform and version, so the specific permissions and runtime conditions have to be verified separately for each target platform; for layout and lifecycle, refer to the [Apple](Apple.md), [Android](Android.md) and [HarmonyOS](HarmonyOS.md) volumes, and do not mask a permission denial with retries.

## Acceptance checklist

- [ ] Entering the same media from the home screen, search, details and an external command locates the same media and playback target.
- [ ] When server capabilities are insufficient, actions give the reason they are unavailable; required features are not hidden because the screen is narrow.
- [ ] The remote can complete sign-in, search, episode selection, playback, subtitle selection and exit, and focus has no dead ends.
- [ ] Returning from details to a long list restores the correct item; after the list refreshes or reflows, a different item at the same position is not restored instead.
- [ ] Closing the settings window does not stop the app-level player, and switching the device layout does not start playback again.
- [ ] UI, CLI, JSON and MCP operations on the same player_id go through the same command handling and result synchronization.
- [ ] Remote requests do not pose as local playback, and reports from old sessions and expired sign-in codes do not pollute the new state.
- [ ] Test separately: directly playable, container change needed, audio/video transcoding needed, subtitles changing the method, and completely unsupported.
- [ ] Playback started/progress/stopped matches the real executor; protocol tests are not replaced by UI screenshots.
- [ ] Light and dark mode, high contrast, long titles, enlarged fonts, screen readers and readability at TV viewing distance pass the corresponding platform checks.
- [ ] Emby and Jellyfin each get protocol tests against explicitly recorded server versions, without assuming the APIs stay the same forever.

## Source verification record

The relevant body text of the following official pages was read on 2026-10-09; the REST guide among them contains historical examples, so before API integration the formal schema of the servers to be supported should still be matched.

| Source | Scope or version boundary |
| --- | --- |
| [Emby Home Screen](https://emby.media/support/articles/apps/common/User-Prefs-HomeScreen.html) | The page explicitly covers Server 4.10.x and later |
| [Emby Plugin UI](https://dev.emby.media/doc/plugins/ui/index.html) | Server plugin configuration interface |
| [Emby Accessibility](https://emby.media/support/articles/apps/windows/features/Win-Feature-Accessibility.html) | The specific Windows client and its theme conditions |
| [Emby Remote Control](https://dev.emby.media/doc/restapi/Remote-Control.html) | Session enumeration, capabilities and remote control requests |
| [Emby Web Socket](https://dev.emby.media/doc/restapi/Web-Socket.html) | Notifications and remote control messages; not a Bern public protocol |
| [Emby Playback Check-ins](https://dev.emby.media/doc/restapi/Playback-Check-ins.html) | Playback state reports |
| [Jellyfin Branding](https://jellyfin.org/docs/general/contributing/branding/) | Brand rules |
| [Jellyfin UX](https://github.com/jellyfin/jellyfin-ux) | README of the official asset repository; not all assets were reviewed on this basis |
| [Jellyfin Style Guides](https://jellyfin.org/docs/general/style-guides/) | Code style notes |
| [Jellyfin Clients](https://jellyfin.org/docs/general/clients/) | Range of clients and the directory's inclusion criteria |
| [Jellyfin CSS Customization](https://jellyfin.org/docs/general/clients/css-customization/) | Scope of custom CSS in Jellyfin Web |
| [Jellyfin Quick Connect](https://jellyfin.org/docs/general/server/quick-connect/) | Pairing sign-in behavior and differences in client capabilities |
| [Jellyfin Codec Support](https://jellyfin.org/docs/general/clients/codec-support/) | Principles for judging compatibility; the results in its tables cannot replace runtime detection on the device |

Server integration testing, real playback and client visual tests have not been done yet. The checklist items above are for acceptance of the later implementation.
