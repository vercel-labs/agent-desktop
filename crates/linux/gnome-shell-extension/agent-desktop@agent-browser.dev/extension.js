import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'dev.agentbrowser.AgentDesktop';
const OBJECT_PATH = '/dev/agentbrowser/AgentDesktop';
const PROTOCOL_VERSION = 1;
const INTERFACE = `<node>
  <interface name="dev.agentbrowser.AgentDesktop.Shell">
    <property name="Version" type="u" access="read"/>
    <method name="ListWindows">
      <arg type="s" name="windows" direction="out"/>
    </method>
  </interface>
</node>`;

function rectOf(r) {
    return {x: r.x, y: r.y, width: r.width, height: r.height};
}

/**
 * Every window the compositor manages, with the two rectangles a client's
 * window coordinates can start from: the frame (the visible window) and the
 * buffer (the surface, including client-side shadows).
 */
function listWindows() {
    return global.display.list_all_windows().map(window => ({
        pid: window.get_pid(),
        title: window.get_title() ?? '',
        wayland: window.get_client_type() === Meta.WindowClientType.WAYLAND,
        minimized: window.minimized,
        frame: rectOf(window.get_frame_rect()),
        buffer: rectOf(window.get_buffer_rect()),
    }));
}

export default class AgentDesktopExtension extends Extension {
    enable() {
        this._service = Gio.DBusExportedObject.wrapJSObject(INTERFACE, {
            get Version() {
                return PROTOCOL_VERSION;
            },
            ListWindows() {
                return JSON.stringify(listWindows());
            },
        });
        this._service.export(Gio.DBus.session, OBJECT_PATH);
        this._owner = Gio.bus_own_name_on_connection(
            Gio.DBus.session, BUS_NAME, Gio.BusNameOwnerFlags.NONE, null, null);
    }

    disable() {
        Gio.bus_unown_name(this._owner);
        this._owner = 0;
        this._service.unexport();
        this._service = null;
    }
}
