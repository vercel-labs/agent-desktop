import Cairo from 'cairo';
import Clutter from 'gi://Clutter';
import GdkPixbuf from 'gi://GdkPixbuf';
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import St from 'gi://St';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
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
    <method name="Locate">
      <arg type="s" name="request" direction="in"/>
      <arg type="s" name="start" direction="out"/>
    </method>
    <method name="UpdateCursor">
      <arg type="s" name="request" direction="in"/>
    </method>
  </interface>
</node>`;

const DART = [[1, 5], [29.6, 22.5], [12.7, 23.7], [4.2, 38.4]];
const GLYPH_WIDTH = 32;
const GLYPH_HEIGHT = 40;
const GLYPH_RIM_WIDTH = 6.5;
const RIPPLE_SIZE = 108;
const RIPPLE_CORE_RADIUS = 19;
const RIPPLE_RING_INSET = 4;
const RIPPLE_RING_WIDTH = 3;
const HIGHLIGHT_BORDER_WIDTH = 2.5;
const HIGHLIGHT_CORNER_RADIUS = 8;
const HIGHLIGHT_PADDING = 5;
const LABEL_FONT_POINTS = 12.5;
const LABEL_PILL_HEIGHT = 24;
const LABEL_PILL_PADDING = 10;
const LABEL_PILL_MAX_WIDTH = 360;
const LABEL_SHADOW_OPACITY = 0.31;
const LABEL_SHADOW_OFFSET = 2;
const LABEL_SHADOW_BLUR = 2.5;
const LABEL_OFFSET_X = 20;
const LABEL_OFFSET_Y = 30;
const LABEL_REVEAL_MS = 180;
const IDLE_REST_MS = 6000;
const REST_FADE_MS = 156;
const UNANCHORED_OFFSET = {x: -180, y: 108};
const ARRIVAL_GRACE_MS = 150;
const CONCEAL_LIMIT_SECONDS = 30;

const MAX_SIZE = 8;
const MAX_FRAMES = 600;
const MAX_FRAME_MS = 100;
const MAX_LABEL_CHARS = 512;
const MAX_PATH_CHARS = 1024;
const MAX_IMAGE_PIXELS = 1024;
const MAX_HOTSPOT = 512;
const OPERATIONS = new Set(['enable', 'travel', 'effect', 'caret', 'hide', 'show', 'disable', 'conceal', 'reveal']);
const CAPTURE_OPERATIONS = new Set(['conceal', 'reveal']);
const STYLED_OPERATIONS = new Set(['enable', 'travel', 'effect', 'caret']);

function invalid(message) {
    return new Error(`Invalid cursor request: ${message}`);
}

function finite(value) {
    return typeof value === 'number' && Number.isFinite(value);
}

function color(value, name) {
    if (!Array.isArray(value) || value.length !== 3 ||
        !value.every(channel => finite(channel) && channel >= 0 && channel <= 1))
        throw invalid(`${name} must be three channels between 0 and 1`);
    return value;
}

function point(value, name) {
    if (!finite(value?.x) || !finite(value?.y))
        throw invalid(`${name} needs finite x and y`);
    return {x: value.x, y: value.y};
}

function rect(value, name) {
    if (value === undefined || value === null)
        return null;
    const {x, y} = point(value, name);
    if (!finite(value.width) || !finite(value.height) || value.width < 0 || value.height < 0)
        throw invalid(`${name} needs a finite, non-negative width and height`);
    return {x, y, width: value.width, height: value.height};
}

function image(value, name) {
    if (value === undefined || value === null)
        return null;
    if (typeof value.path !== 'string' || !value.path.startsWith('/') ||
        value.path.length > MAX_PATH_CHARS || value.path.includes('\0'))
        throw invalid(`${name} needs an absolute path`);
    const hotspot = point(value.hotspot, `${name} hotspot`);
    if (hotspot.x < 0 || hotspot.y < 0 || hotspot.x > MAX_HOTSPOT || hotspot.y > MAX_HOTSPOT)
        throw invalid(`${name} hotspot must be between 0 and ${MAX_HOTSPOT}`);
    return {path: value.path, hotspot};
}

function look(value) {
    if (!finite(value?.size) || value.size <= 0 || value.size > MAX_SIZE)
        throw invalid(`size must be above 0 and at most ${MAX_SIZE}`);
    return {
        fill: color(value.fill, 'fill'),
        rim: color(value.rim, 'rim'),
        accent: color(value.accent, 'accent'),
        size: value.size,
        arrow: image(value.arrow, 'arrow'),
        pointer: image(value.pointer, 'pointer'),
    };
}

function label(value) {
    if (value === undefined || value === null)
        return null;
    if (typeof value !== 'string' || value.length > MAX_LABEL_CHARS)
        throw invalid(`label must be a string of at most ${MAX_LABEL_CHARS} characters`);
    return value.trim().length > 0 ? value : null;
}

function frames(value, least, check) {
    if (!Array.isArray(value) || value.length < least || value.length > MAX_FRAMES)
        throw invalid(`frames must hold ${least} to ${MAX_FRAMES} entries`);
    if (!value.every(frame => Array.isArray(frame) && frame.length === 2 && frame.every(check)))
        throw invalid('every frame must be a pair of numbers in range');
    return value;
}

function frameMs(value) {
    if (!finite(value) || value <= 0 || value > MAX_FRAME_MS)
        throw invalid(`frame_ms must be above 0 and at most ${MAX_FRAME_MS}`);
    return value;
}

function owner(request) {
    if (typeof request.session !== 'string' || request.session.length === 0)
        throw invalid('session must be a non-empty string');
    const agent = request.agent ?? null;
    if (agent !== null && (typeof agent !== 'string' || agent.length === 0))
        throw invalid('agent must be a non-empty string or null');
    return {session: request.session, agent};
}

/**
 * Checks one request from the CLI and returns it in the shape the layer uses.
 * Anything malformed is refused before a cursor changes.
 */
function parseRequest(text) {
    const request = JSON.parse(text);
    if (!OPERATIONS.has(request?.op))
        throw invalid(`op must be one of ${[...OPERATIONS].join(', ')}`);
    if (CAPTURE_OPERATIONS.has(request.op))
        return {op: request.op};
    const parsed = {op: request.op, ...owner(request)};
    if (STYLED_OPERATIONS.has(request.op)) {
        parsed.look = look(request.look);
        parsed.label = label(request.label);
    }
    const unit = channel => finite(channel) && channel >= 0 && channel <= 1;
    switch (request.op) {
    case 'travel':
        parsed.frames = frames(request.frames, 1, finite);
        parsed.frameMs = frameMs(request.frame_ms);
        parsed.pointer = request.pointer === true;
        parsed.clearCaret = request.clear_caret === true;
        parsed.drag = request.drag === true;
        break;
    case 'effect':
        parsed.point = point(request.point, 'point');
        parsed.target = rect(request.target, 'target');
        parsed.frames = frames(request.frames, 0, unit);
        parsed.frameMs = frameMs(request.frame_ms);
        break;
    case 'caret':
        parsed.rect = rect(request.rect, 'rect');
        parsed.follow = request.follow === true;
        break;
    }
    return parsed;
}

function parseLocate(text) {
    const request = JSON.parse(text);
    return {...owner(request ?? {}), destination: point(request?.destination, 'destination')};
}

function animationsEnabled() {
    return St.Settings.get().enable_animations;
}

function css(rgb) {
    return `rgb(${rgb.map(channel => Math.round(channel * 255)).join(', ')})`;
}

function monitorIndexAt(at) {
    const index = Main.layoutManager.monitors.findIndex(monitor =>
        at.x >= monitor.x && at.x < monitor.x + monitor.width &&
        at.y >= monitor.y && at.y < monitor.y + monitor.height);
    return index >= 0 ? index : Main.layoutManager.primaryIndex;
}

/**
 * Where the label pill goes: right of and below the tip when it fits on the
 * screen, mirrored otherwise, and never off the screen. The same rule as
 * core's `place_label`, which the macOS and Windows overlays use.
 */
function placeLabel(tip, [width, height], screen) {
    const right = screen.x + screen.width;
    const bottom = screen.y + screen.height;
    const preferredX = tip.x + LABEL_OFFSET_X;
    const preferredY = tip.y + LABEL_OFFSET_Y;
    const x = preferredX + width <= right ? preferredX : tip.x - width - LABEL_OFFSET_X;
    const y = preferredY + height <= bottom ? preferredY : tip.y - height - LABEL_OFFSET_Y;
    const clamp = (value, low, high) => Math.min(Math.max(value, low), Math.max(high, low));
    return {
        x: clamp(x, screen.x, right - width),
        y: clamp(y, screen.y, bottom - height),
        width: Math.min(width, Math.max(screen.width, 0)),
        height: Math.min(height, Math.max(screen.height, 0)),
    };
}

function paintDart(area, style) {
    const cr = area.get_context();
    cr.scale(style.size, style.size);
    cr.translate(GLYPH_RIM_WIDTH, GLYPH_RIM_WIDTH);
    const [first, ...rest] = DART;
    cr.moveTo(...first);
    for (const corner of rest)
        cr.lineTo(...corner);
    cr.closePath();
    cr.setLineJoin(Cairo.LineJoin.ROUND);
    cr.setLineWidth(GLYPH_RIM_WIDTH);
    cr.setSourceRGB(...style.rim);
    cr.strokePreserve();
    cr.setSourceRGB(...style.fill);
    cr.fill();
    cr.$dispose();
}

function paintRipple(area, style, phase) {
    const cr = area.get_context();
    if (phase > 0 && phase <= 1) {
        const scale = style.size;
        const centre = RIPPLE_SIZE * scale / 2;
        const fade = 1 - phase;
        cr.arc(centre, centre, RIPPLE_CORE_RADIUS * scale * (1 - phase * 0.4), 0, 2 * Math.PI);
        cr.setSourceRGBA(...style.accent, 0.55 * fade);
        cr.fill();
        const ring = (RIPPLE_SIZE / 2 - RIPPLE_RING_INSET) * scale * phase;
        const width = Math.min(RIPPLE_RING_WIDTH * scale, ring);
        if (width > 0) {
            cr.arc(centre, centre, ring - width / 2, 0, 2 * Math.PI);
            cr.setLineWidth(width);
            cr.setSourceRGBA(...style.accent, 0.85 * fade);
            cr.stroke();
        }
    }
    cr.$dispose();
}

/**
 * A custom cursor image as St draws it, or null when the file is not a
 * readable image within the size limit, in which case the dart is drawn.
 */
function loadFace(face, scale) {
    if (!face)
        return null;
    const [format, width, height] = GdkPixbuf.Pixbuf.get_file_info(face.path);
    if (!format || width <= 0 || height <= 0 || width > MAX_IMAGE_PIXELS || height > MAX_IMAGE_PIXELS)
        return null;
    const uri = GLib.filename_to_uri(face.path, null);
    return {
        style: `background-image: url("${uri}"); background-size: ${width * scale}px ${height * scale}px;`,
        width: Math.ceil(width * scale),
        height: Math.ceil(height * scale),
        hotspot: face.hotspot,
    };
}

class Cursor {
    constructor() {
        this.group = new St.Widget({reactive: false, can_focus: false, track_hover: false});
        this._caret = new St.Widget({reactive: false, visible: false});
        this._highlight = new St.Widget({reactive: false, visible: false});
        this._ripple = new St.DrawingArea({reactive: false, visible: false});
        this._text = new St.Label({
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
            x_expand: true,
            y_expand: true,
        });
        this._text.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        this._pill = new St.Bin({reactive: false, visible: false, child: this._text});
        this._glyph = new St.DrawingArea({reactive: false});
        this._image = new St.Widget({reactive: false, visible: false});
        for (const actor of [this._caret, this._highlight, this._ripple, this._pill, this._glyph, this._image])
            this.group.add_child(actor);
        Main.layoutManager.uiGroup.add_child(this.group);
        this._glyph.connect('repaint', area => paintDart(area, this._style));
        this._ripple.connect('repaint', area => paintRipple(area, this._style, this._ripplePhase));
        this._ripplePhase = 0;
        this._style = null;
        this._faceKey = null;
        this._faces = {arrow: null, pointer: null};
        this._face = null;
        this._pointing = false;
        this._label = null;
        this._revealPending = false;
        this._caretShown = false;
        this._travel = null;
        this._effect = null;
        this._restTimeout = 0;
        this.tip = null;
        this.anchored = false;
        this.dragging = false;
        this.moves = 0;
        this.hidden = false;
        this.concealed = false;
        this.resting = false;
    }

    get visible() {
        return this.group.visible;
    }

    _updateVisibility() {
        this.group.visible = !this.hidden && !this.concealed;
    }

    restyle(style) {
        this._style = style;
        const scale = style.size;
        const rim = GLYPH_RIM_WIDTH * scale;
        this._glyph.set_size(Math.ceil(GLYPH_WIDTH * scale + 2 * rim), Math.ceil(GLYPH_HEIGHT * scale + 2 * rim));
        this._glyph.queue_repaint();
        this._ripple.set_size(Math.ceil(RIPPLE_SIZE * scale), Math.ceil(RIPPLE_SIZE * scale));
        this._highlight.style = `border: ${HIGHLIGHT_BORDER_WIDTH * scale}px solid ${css(style.accent)}; ` +
            `border-radius: ${HIGHLIGHT_CORNER_RADIUS * scale}px;`;
        this._caret.style = `background-color: ${css(style.accent)};`;
        this._pill.style = `background-color: black; border-radius: ${LABEL_PILL_HEIGHT * scale / 2}px; ` +
            `padding: 0 ${LABEL_PILL_PADDING * scale}px; box-shadow: 0 ${LABEL_SHADOW_OFFSET * scale}px ` +
            `${LABEL_SHADOW_BLUR * scale * 2}px rgba(0, 0, 0, ${LABEL_SHADOW_OPACITY});`;
        this._text.style = `color: white; font-size: ${LABEL_FONT_POINTS * scale}px;`;
        const faceKey = JSON.stringify([style.arrow, style.pointer, scale]);
        if (faceKey !== this._faceKey) {
            this._faceKey = faceKey;
            this._faces = {arrow: loadFace(style.arrow, scale), pointer: loadFace(style.pointer, scale)};
        }
        this._showFace();
    }

    _showFace() {
        const face = this._pointing && this._faces.pointer ? this._faces.pointer : this._faces.arrow;
        this._face = face;
        const placed = this.tip !== null;
        this._glyph.visible = placed && !face;
        this._image.visible = placed && Boolean(face);
        if (face) {
            this._image.style = face.style;
            this._image.set_size(face.width, face.height);
        }
        if (this.tip)
            this.place(this.tip);
    }

    place(tip) {
        const first = this.tip === null;
        this.tip = tip;
        if (first) {
            this._glyph.visible = !this._face;
            this._image.visible = Boolean(this._face);
        }
        const scale = this._style.size;
        const rim = GLYPH_RIM_WIDTH * scale;
        this._glyph.set_position(Math.round(tip.x - DART[0][0] * scale - rim),
            Math.round(tip.y - DART[0][1] * scale - rim));
        if (this._face) {
            this._image.set_position(Math.round(tip.x - this._face.hotspot.x * scale),
                Math.round(tip.y - this._face.hotspot.y * scale));
        }
        const half = RIPPLE_SIZE * scale / 2;
        this._ripple.set_position(Math.round(tip.x - half), Math.round(tip.y - half));
        this._placeLabel();
    }

    _placeLabel() {
        if (!this._label) {
            this._pill.hide();
            return;
        }
        const scale = this._style.size;
        const padding = LABEL_PILL_PADDING * scale;
        this._text.ensure_style();
        const [, natural] = this._text.clutter_text.get_preferred_width(-1);
        const textWidth = Math.min(Math.max(natural, 0), LABEL_PILL_MAX_WIDTH * scale - padding * 2);
        const screen = Main.layoutManager.getWorkAreaForMonitor(monitorIndexAt(this.tip));
        const placed = placeLabel(this.tip, [textWidth + padding * 2, LABEL_PILL_HEIGHT * scale], screen);
        this._pill.set_size(Math.ceil(placed.width), Math.ceil(placed.height));
        this._pill.set_position(Math.round(placed.x), Math.round(placed.y));
        this._pill.show();
    }

    setLabel(text) {
        if (text === this._label)
            return;
        this._label = text;
        this._pill.remove_transition('opacity');
        if (text) {
            this._text.text = text;
            this._pill.opacity = 0;
            this._revealPending = true;
        } else {
            this._revealPending = false;
        }
        if (this.tip)
            this._placeLabel();
    }

    _reveal() {
        if (!this._revealPending)
            return;
        this._revealPending = false;
        this._pill.opacity = 0;
        if (!animationsEnabled()) {
            this._pill.opacity = 255;
            return;
        }
        this._pill.ease({opacity: 255, duration: LABEL_REVEAL_MS, mode: Clutter.AnimationMode.EASE_OUT_CUBIC});
    }

    wake() {
        if (this._restTimeout)
            GLib.source_remove(this._restTimeout);
        this.group.remove_transition('opacity');
        this.group.opacity = 255;
        this.resting = false;
        Main.layoutManager.uiGroup.set_child_above_sibling(this.group, null);
        this._restTimeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, IDLE_REST_MS, () => {
            this._restTimeout = 0;
            this._rest();
            return GLib.SOURCE_REMOVE;
        });
    }

    _rest() {
        this.resting = true;
        const cleared = () => this._hideCaret();
        if (!animationsEnabled()) {
            this.group.opacity = 0;
            cleared();
            return;
        }
        this.group.ease({
            opacity: 0,
            duration: REST_FADE_MS,
            mode: Clutter.AnimationMode.LINEAR,
            onComplete: cleared,
        });
    }

    _hideCaret() {
        this._caret.hide();
        this._caretShown = false;
    }

    enable(request) {
        this.wake();
        this.restyle(request.look);
        this.moves = 0;
        this.dragging = false;
        this.setLabel(request.label);
        if (!this.tip) {
            const [x, y] = global.get_pointer();
            this.place({x, y});
        }
        this.anchored = true;
        this.hidden = false;
        this._updateVisibility();
        this._reveal();
    }

    /**
     * Plays the path the CLI computed and calls `done` once the cursor stands
     * on its last frame, which is when the CLI dispatches the action. Frames
     * are picked by elapsed time, so a dropped frame changes smoothness and
     * never the arrival instant. A timeout past the path's length answers
     * even if the stage stops producing frames.
     */
    travel(request, done) {
        this.finishTravel();
        this.wake();
        this.restyle(request.look);
        this.setLabel(request.label);
        if (request.clearCaret)
            this._hideCaret();
        this._pointing = false;
        this._showFace();
        this.moves += 1;
        this.anchored = true;
        this.dragging = request.drag;
        const path = request.frames;
        const at = index => ({x: path[index][0], y: path[index][1]});
        const duration = Math.round((path.length - 1) * request.frameMs);
        const travel = {done, timeline: null, timeout: 0};
        travel.arrive = () => {
            if (this._travel !== travel)
                return;
            this._travel = null;
            travel.timeline?.stop();
            if (travel.timeout)
                GLib.source_remove(travel.timeout);
            this._pointing = request.pointer;
            this._showFace();
            this.place(at(path.length - 1));
            this._reveal();
            travel.done();
        };
        this._travel = travel;
        this.place(at(0));
        if (duration <= 0 || !animationsEnabled()) {
            travel.arrive();
            return;
        }
        travel.timeline = new Clutter.Timeline({actor: global.stage, duration});
        travel.timeline.connect('new-frame', (_timeline, elapsed) => {
            if (this._travel === travel)
                this.place(at(Math.min(Math.floor(elapsed / request.frameMs), path.length - 1)));
        });
        travel.timeline.connect('completed', () => travel.arrive());
        travel.timeout = GLib.timeout_add(GLib.PRIORITY_HIGH, duration + ARRIVAL_GRACE_MS, () => {
            travel.timeout = 0;
            travel.arrive();
            return GLib.SOURCE_REMOVE;
        });
        travel.timeline.start();
    }

    finishTravel() {
        this._travel?.arrive();
    }

    /**
     * The click ripple and the outline around the acted-on element, after the
     * action dispatched. Nothing waits on it, so the CLI is answered first and
     * a newer request takes it over.
     */
    effect(request) {
        this.finishTravel();
        this._stopEffect();
        this.wake();
        this.restyle(request.look);
        this.setLabel(request.label);
        this.dragging = false;
        this.place(request.point);
        this._reveal();
        const path = request.frames;
        if (path.length === 0 || !animationsEnabled())
            return;
        const scale = this._style.size;
        const target = request.target;
        if (target) {
            const padding = HIGHLIGHT_PADDING * scale;
            this._highlight.set_position(Math.round(target.x - padding), Math.round(target.y - padding));
            this._highlight.set_size(Math.ceil(target.width + padding * 2), Math.ceil(target.height + padding * 2));
        }
        const show = index => {
            const [ripple, highlight] = path[index];
            this._ripplePhase = ripple;
            this._ripple.visible = ripple > 0 && ripple < 1;
            this._ripple.queue_repaint();
            this._highlight.visible = Boolean(target) && highlight > 0;
            this._highlight.opacity = Math.round(255 * highlight);
        };
        show(0);
        const effect = new Clutter.Timeline({
            actor: global.stage,
            duration: Math.max(1, Math.round((path.length - 1) * request.frameMs)),
        });
        effect.connect('new-frame', (_timeline, elapsed) =>
            show(Math.min(Math.floor(elapsed / request.frameMs), path.length - 1)));
        effect.connect('completed', () => this._stopEffect());
        this._effect = effect;
        effect.start();
    }

    _stopEffect() {
        this._effect?.stop();
        this._effect = null;
        this._ripple.hide();
        this._highlight.hide();
    }

    /**
     * Places the agent's text caret, or with `follow` moves only a caret that
     * is already showing. A follow while the overlay rests leaves it resting.
     */
    caret(request) {
        if (request.follow && this.resting)
            return;
        this.wake();
        this.restyle(request.look);
        if (request.follow && !this._caretShown)
            return;
        if (request.rect) {
            const {x, y, width, height} = request.rect;
            this._caret.set_position(Math.round(x), Math.round(y));
            this._caret.set_size(Math.max(1, Math.round(width)), Math.max(1, Math.round(height)));
            this._caret.show();
            this._caretShown = true;
        } else if (request.follow) {
            this._hideCaret();
        }
    }

    /**
     * Dispatch hides the cursors around every headed command. A hide that
     * cancels a drag takes the cursor off screen and forgets where it stands,
     * so the next travel does not set off from the stale grab point. Any other
     * hide leaves it where it landed and the next travel glides on from there,
     * as on macOS.
     */
    hide() {
        if (!this.dragging)
            return;
        this.dragging = false;
        this.hidden = true;
        this.anchored = false;
        this._hideCaret();
        this._updateVisibility();
    }

    show() {
        this.wake();
        this.hidden = false;
        this._updateVisibility();
        this._reveal();
    }

    destroy() {
        this.finishTravel();
        this._stopEffect();
        if (this._restTimeout)
            GLib.source_remove(this._restTimeout);
        this._restTimeout = 0;
        this.group.destroy();
    }
}

/**
 * The agent cursors drawn above every window, one per session and agent id.
 * The cursors never take input, so pointer events reach the windows below.
 */
class CursorLayer {
    constructor() {
        this._cursors = [];
        this._concealDepth = 0;
        this._concealTimeout = 0;
    }

    _find(session, agent) {
        return this._cursors.find(entry => entry.session === session && entry.agent === agent);
    }

    _matching(session, agent, everyAgent) {
        return this._cursors.filter(entry =>
            entry.session === session && (everyAgent || entry.agent === agent));
    }

    _owned(request) {
        let entry = this._find(request.session, request.agent);
        if (!entry) {
            entry = {session: request.session, agent: request.agent, cursor: new Cursor()};
            entry.cursor.concealed = this._concealDepth > 0;
            entry.cursor.restyle(request.look);
            entry.cursor._updateVisibility();
            this._cursors.push(entry);
        }
        return entry.cursor;
    }

    /**
     * Where a travel to `destination` sets off: the cursor's tip, or for a
     * cursor with no known position, a point below and left of the
     * destination on the same screen, the way the macOS overlay starts.
     */
    locate(request) {
        const cursor = this._find(request.session, request.agent)?.cursor;
        const moves = cursor?.moves ?? 0;
        if (cursor?.anchored && cursor.tip)
            return {x: cursor.tip.x, y: cursor.tip.y, moves};
        const screen = Main.layoutManager.monitors[monitorIndexAt(request.destination)];
        const clamp = (value, low, size) => Math.min(Math.max(value, low), low + size);
        return {
            x: clamp(request.destination.x + UNANCHORED_OFFSET.x, screen.x, screen.width),
            y: clamp(request.destination.y + UNANCHORED_OFFSET.y, screen.y, screen.height),
            moves,
        };
    }

    update(request, done) {
        switch (request.op) {
        case 'conceal':
            this._conceal();
            break;
        case 'reveal':
            this._reveal();
            break;
        case 'disable':
            for (const entry of this._matching(request.session, null, true))
                entry.cursor.destroy();
            this._cursors = this._cursors.filter(entry => entry.session !== request.session);
            break;
        case 'hide':
        case 'show':
            for (const entry of this._matching(request.session, request.agent, request.agent === null))
                entry.cursor[request.op]();
            break;
        case 'travel':
            this._owned(request).travel(request, done);
            return;
        case 'caret':
            if (request.follow && !this._find(request.session, request.agent))
                break;
            this._owned(request).caret(request);
            break;
        default:
            this._owned(request)[request.op](request);
        }
        done();
    }

    /**
     * Hides every cursor for one capture. Captures can overlap, so each
     * conceal is counted and the cursors come back only when every capture
     * has revealed, or when the newest conceal is older than the limit, which
     * covers a capture whose process never asked.
     */
    _conceal() {
        this._concealDepth += 1;
        if (this._concealTimeout)
            GLib.source_remove(this._concealTimeout);
        this._concealTimeout = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, CONCEAL_LIMIT_SECONDS, () => {
            this._concealTimeout = 0;
            this._concealDepth = 0;
            this._setConcealed(false);
            return GLib.SOURCE_REMOVE;
        });
        this._setConcealed(true);
    }

    _reveal() {
        if (this._concealDepth === 0)
            return;
        this._concealDepth -= 1;
        if (this._concealDepth > 0)
            return;
        if (this._concealTimeout) {
            GLib.source_remove(this._concealTimeout);
            this._concealTimeout = 0;
        }
        this._setConcealed(false);
    }

    _setConcealed(concealed) {
        for (const entry of this._cursors) {
            entry.cursor.concealed = concealed;
            entry.cursor._updateVisibility();
        }
    }

    destroy() {
        if (this._concealTimeout) {
            GLib.source_remove(this._concealTimeout);
            this._concealTimeout = 0;
        }
        this._concealDepth = 0;
        for (const entry of this._cursors)
            entry.cursor.destroy();
        this._cursors = [];
    }
}

function rectOf(r) {
    return {x: r.x, y: r.y, width: r.width, height: r.height};
}

/**
 * The frame of an Xwayland window in the X11 coordinates its client reports.
 * With a fractional monitor scale Xwayland can scale its clients, so these
 * differ from the stage's logical coordinates. A shell without the conversion
 * does not scale Xwayland clients, so the frame is unchanged there.
 */
function protocolFrame(window) {
    const frame = window.get_frame_rect();
    return rectOf(typeof window.stage_to_protocol_rect === 'function'
        ? window.stage_to_protocol_rect(frame)
        : frame);
}

/**
 * Every window the compositor manages, with the two rectangles a client's
 * window coordinates can start from: the frame (the visible window) and the
 * buffer (the surface, including client-side shadows). An Xwayland window
 * also carries its frame in X11 coordinates.
 */
function listWindows() {
    return global.display.list_all_windows().map(window => {
        const wayland = window.get_client_type() === Meta.WindowClientType.WAYLAND;
        return {
            pid: window.get_pid(),
            title: window.get_title() ?? '',
            wayland,
            minimized: window.minimized,
            frame: rectOf(window.get_frame_rect()),
            buffer: rectOf(window.get_buffer_rect()),
            protocol: wayland ? null : protocolFrame(window),
        };
    });
}

export default class AgentDesktopExtension extends Extension {
    enable() {
        this._layer = new CursorLayer();
        const layer = this._layer;
        this._service = Gio.DBusExportedObject.wrapJSObject(INTERFACE, {
            get Version() {
                return PROTOCOL_VERSION;
            },
            ListWindows() {
                return JSON.stringify(listWindows());
            },
            Locate(text) {
                return JSON.stringify(layer.locate(parseLocate(text)));
            },
            UpdateCursorAsync([text], invocation) {
                let request;
                try {
                    request = parseRequest(text);
                } catch (error) {
                    invocation.return_dbus_error('org.freedesktop.DBus.Error.InvalidArgs', error.message);
                    return;
                }
                let answered = false;
                const done = () => {
                    if (!answered) {
                        answered = true;
                        invocation.return_value(null);
                    }
                };
                try {
                    layer.update(request, done);
                } catch (error) {
                    if (!answered) {
                        answered = true;
                        invocation.return_dbus_error('org.freedesktop.DBus.Error.Failed', error.message);
                    }
                }
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
        this._layer.destroy();
        this._layer = null;
    }
}
