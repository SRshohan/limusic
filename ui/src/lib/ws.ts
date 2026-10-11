// Single WebSocket to crates/server's /ws (openspec/changes/signed-in-http-parity, tasks 2.5/12.2)
// — session-scoped events (auth-changed, account-selection-required, lastfm-state). Not used for
// Listen Together: that's a direct connection to a separate relay process, see lt-client.ts.
// Same dev/prod split as api.web.ts's SERVER_URL: dev talks cross-origin to :8787, production
// goes through the same hostname Caddy serves the UI from (so the session cookie's SameSite=Lax
// is satisfied either way).
const WS_URL = import.meta.env.DEV
	? 'ws://localhost:8787/ws'
	: `${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/ws`;
/** How long to wait before retrying after a drop, while something is still listening. */
const RECONNECT_DELAY_MS = 2000;

type Listener = (data: unknown) => void;

const listeners = new Map<string, Set<Listener>>();
let socket: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

function connect() {
	if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
		return;
	}
	// The session cookie rides on the browser's own cookie jar for this origin — no header to set
	// by hand, same as every other fetch in this file.
	socket = new WebSocket(WS_URL);
	socket.addEventListener('message', (ev) => {
		let frame: { type?: string; data?: unknown };
		try {
			frame = JSON.parse(ev.data);
		} catch {
			return;
		}
		if (!frame.type) return;
		const set = listeners.get(frame.type);
		if (set) for (const cb of set) cb(frame.data);
	});
	socket.addEventListener('close', () => {
		socket = null;
		// Only worth reconnecting while someone is still listening — no point holding a socket
		// open for events nothing on screen cares about anymore.
		if (listeners.size > 0 && !reconnectTimer) {
			reconnectTimer = setTimeout(() => {
				reconnectTimer = null;
				connect();
			}, RECONNECT_DELAY_MS);
		}
	});
}

/** Subscribe to one event type's frames. Opens the connection lazily on first subscriber, closes
 *  nothing itself (the server/browser tear it down); returns an unlisten matching every other
 *  `onX` in this command surface. */
export function subscribeWs(eventType: string, cb: Listener): () => void {
	let set = listeners.get(eventType);
	if (!set) {
		set = new Set();
		listeners.set(eventType, set);
	}
	set.add(cb);
	connect();
	return () => {
		set.delete(cb);
		if (set.size === 0) listeners.delete(eventType);
	};
}
