// Listen Together client for the browser/PWA build (openspec/changes/signed-in-http-parity, task
// 12.10). There is no Rust server-side counterpart to port here: `crates/sync-server` is already a
// standalone relay process (see src-tauri/src/listentogether/mod.rs's own header comment), so this
// is a from-scratch TypeScript port of that file's connection/state machine, talking the same JSON
// wire protocol (`crates/listen-protocol`) over a WebSocket opened directly to the relay — not
// `ws.ts`'s `/ws` (that's a separate connection to `crates/server` for session-scoped events).
//
// Scope cut vs. the desktop client: the desktop version also drives local mpv playback (the
// `SyncCommand`/bridge-task machinery) because `AppState` owns a real player. This server has no
// playback yet (Phase 3, a separate change) — `current_track`/`queue` are still mirrored for
// display, but `sync_playback`'s play/pause/seek/volume kinds have nothing to apply to here, so
// they're intentionally dropped rather than stubbed.
import type { LtState, LtUser, LtTrack, LtSuggestion } from './api.types';

/** Same default as `src-tauri/src/listentogether/mod.rs`'s `DEFAULT_SERVER` — kept out of the UI
 *  until the user sets their own (see `LtState.defaultServerUrl`'s doc comment in api.types.ts). */
const DEFAULT_SERVER_URL = 'wss://fedora-1.tail9c4985.ts.net/ws';
const PING_INTERVAL_MS = 25_000;
/** Mirrors `backoff_delay` (context/19 Part B §1): 1,2,4,8,16,32s. */
const RECONNECT_BACKOFF_MS = [1000, 2000, 4000, 8000, 16000, 32000];
const MAX_RECONNECT_ATTEMPTS = 15;

// --- wire types (crates/listen-protocol, `#[serde(tag = "type", rename_all = "snake_case")]`) ---

type ClientMessage =
	| { type: 'create_room'; username: string }
	| { type: 'join_room'; room_code: string; username: string }
	| { type: 'reconnect'; session_token: string }
	| { type: 'leave_room' }
	| { type: 'approve_join'; user_id: string }
	| { type: 'reject_join'; user_id: string }
	| { type: 'kick_user'; user_id: string }
	| { type: 'transfer_host'; user_id: string }
	| { type: 'approve_suggestion'; id: string }
	| { type: 'reject_suggestion'; id: string }
	| { type: 'request_sync' }
	| { type: 'ping' }
	| ({ type: 'playback' } & PlaybackWire);

interface RoomStateWire {
	room_code: string;
	host_id: string;
	users: LtUser[];
	current_track: LtTrack | null;
	is_playing: boolean;
	position_ms: number;
	last_update_ms: number;
	volume: number;
	queue: LtTrack[];
}

export interface PlaybackWire {
	kind: 'play' | 'pause' | 'seek' | 'change_track' | 'sync_queue' | 'set_volume';
	position_ms: number;
	server_time_ms: number;
	track?: LtTrack | null;
	queue?: LtTrack[] | null;
	playing: boolean;
	volume: number;
}

type ServerMessage =
	| { type: 'room_created'; room_code: string; user_id: string; session_token: string; state: RoomStateWire }
	| { type: 'join_request'; user_id: string; username: string }
	| { type: 'join_approved'; user_id: string; session_token: string; state: RoomStateWire }
	| { type: 'join_rejected'; reason: string }
	| { type: 'reconnected'; user_id: string; is_host: boolean; state: RoomStateWire }
	| { type: 'user_joined'; user: LtUser }
	| { type: 'user_left'; user_id: string }
	| { type: 'user_disconnected'; user_id: string }
	| { type: 'user_reconnected'; user_id: string }
	| { type: 'host_changed'; host_id: string }
	| ({ type: 'sync_playback' } & PlaybackWire)
	| { type: 'sync_state'; state: RoomStateWire }
	| { type: 'suggestion_received'; suggestion: LtSuggestion }
	| { type: 'suggestion_approved'; id: string }
	| { type: 'suggestion_rejected'; id: string; reason: string }
	| { type: 'kicked'; reason: string }
	| { type: 'error'; code: string; message: string }
	| { type: 'pong' };

// --- local state machine ------------------------------------------------------------------

type StateListener = (s: LtState) => void;
type NoticeListener = (msg: string) => void;

function initialState(): LtState {
	return {
		status: 'disconnected',
		role: 'none',
		requesting: false,
		roomCode: null,
		myId: null,
		serverUrl: '',
		defaultServerUrl: DEFAULT_SERVER_URL,
		users: [],
		currentTrack: null,
		queue: [],
		pendingJoins: [],
		suggestions: []
	};
}

let state: LtState = initialState();
let sessionToken: string | null = null;
let ws: WebSocket | null = null;
let pingTimer: ReturnType<typeof setInterval> | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectAttempt = 0;
/** Bumped on every leave/new-connect to invalidate callbacks from a superseded socket — mirrors
 *  the desktop client's `AtomicU64 gen` (comment there explains why: a late arrival from a
 *  cancelled connect must never touch state a newer connection already owns). */
let connectionGen = 0;

const stateListeners = new Set<StateListener>();
const noticeListeners = new Set<NoticeListener>();

function emitState() {
	for (const cb of stateListeners) cb(state);
}
function emitNotice(msg: string) {
	for (const cb of noticeListeners) cb(msg);
}

function absorb(s: RoomStateWire) {
	state = { ...state, roomCode: s.room_code, users: s.users, currentTrack: s.current_track, queue: s.queue };
}

function send(msg: ClientMessage) {
	if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(msg));
}

function resetRoom() {
	state = {
		...state,
		status: 'disconnected',
		role: 'none',
		requesting: false,
		roomCode: null,
		myId: null,
		users: [],
		currentTrack: null,
		queue: [],
		pendingJoins: [],
		suggestions: []
	};
	sessionToken = null;
}

function teardownSocket() {
	if (pingTimer) {
		clearInterval(pingTimer);
		pingTimer = null;
	}
	if (reconnectTimer) {
		clearTimeout(reconnectTimer);
		reconnectTimer = null;
	}
	if (ws) {
		ws.onopen = null;
		ws.onmessage = null;
		ws.onclose = null;
		ws.onerror = null;
		try {
			ws.close();
		} catch {
			/* already closed */
		}
		ws = null;
	}
}

function start(initial: ClientMessage) {
	connectionGen += 1;
	const gen = connectionGen;
	teardownSocket();
	resetRoom();
	state = { ...state, status: 'connecting', requesting: true };
	emitState();
	reconnectAttempt = 0;
	openSocket(gen, initial);
}

function openSocket(gen: number, initial: ClientMessage) {
	if (gen !== connectionGen) return;
	let socket: WebSocket;
	try {
		socket = new WebSocket(state.serverUrl || DEFAULT_SERVER_URL);
	} catch {
		maybeReconnect(gen);
		return;
	}
	ws = socket;
	socket.onopen = () => {
		if (gen !== connectionGen) {
			socket.close();
			return;
		}
		reconnectAttempt = 0;
		state = { ...state, status: 'connected' };
		send(initial);
		emitState();
		pingTimer = setInterval(() => send({ type: 'ping' }), PING_INTERVAL_MS);
	};
	socket.onmessage = (ev) => {
		if (gen !== connectionGen) return;
		let msg: ServerMessage;
		try {
			msg = JSON.parse(ev.data);
		} catch {
			return;
		}
		handleServerMessage(msg, gen);
	};
	socket.onclose = () => {
		if (gen !== connectionGen) return;
		ws = null;
		if (pingTimer) {
			clearInterval(pingTimer);
			pingTimer = null;
		}
		maybeReconnect(gen);
	};
}

function maybeReconnect(gen: number) {
	// Reconnect only if we were ever in a room (have a token) — a fresh create/join that never
	// landed just fails, same as the desktop client.
	if (!sessionToken) {
		closeLocally("Couldn't reach the Listen Together server.");
		return;
	}
	reconnectAttempt += 1;
	if (reconnectAttempt > MAX_RECONNECT_ATTEMPTS) {
		closeLocally('Lost connection to the room.');
		return;
	}
	state = { ...state, status: 'connecting' };
	emitState();
	const delay = RECONNECT_BACKOFF_MS[Math.min(reconnectAttempt - 1, RECONNECT_BACKOFF_MS.length - 1)];
	const token = sessionToken;
	reconnectTimer = setTimeout(() => {
		reconnectTimer = null;
		if (gen !== connectionGen) return;
		openSocket(gen, { type: 'reconnect', session_token: token });
	}, delay);
}

function closeLocally(notice: string) {
	if (notice) emitNotice(notice);
	connectionGen += 1;
	teardownSocket();
	resetRoom();
	emitState();
}

function handleServerMessage(sm: ServerMessage, _gen: number) {
	switch (sm.type) {
		case 'pong':
			return;
		case 'room_created':
			sessionToken = sm.session_token;
			state = { ...state, role: 'host', requesting: false, myId: sm.user_id };
			absorb(sm.state);
			break;
		case 'join_request':
			if (!state.pendingJoins.some((p) => p.userId === sm.user_id)) {
				state = {
					...state,
					pendingJoins: [...state.pendingJoins, { userId: sm.user_id, username: sm.username }]
				};
				emitNotice(`${sm.username} wants to join`);
			}
			break;
		case 'join_approved':
			sessionToken = sm.session_token;
			state = { ...state, role: 'guest', requesting: false, myId: sm.user_id };
			absorb(sm.state);
			break;
		case 'join_rejected':
			closeLocally(sm.reason);
			return;
		case 'reconnected':
			state = { ...state, role: sm.is_host ? 'host' : 'guest', requesting: false, myId: sm.user_id };
			absorb(sm.state);
			break;
		case 'user_joined':
			state = {
				...state,
				pendingJoins: state.pendingJoins.filter((p) => p.userId !== sm.user.user_id),
				users: state.users.some((u) => u.user_id === sm.user.user_id)
					? state.users
					: [...state.users, sm.user]
			};
			break;
		case 'user_left':
			state = {
				...state,
				users: state.users.filter((u) => u.user_id !== sm.user_id),
				pendingJoins: state.pendingJoins.filter((p) => p.userId !== sm.user_id)
			};
			break;
		case 'user_disconnected':
			state = {
				...state,
				users: state.users.map((u) => (u.user_id === sm.user_id ? { ...u, is_connected: false } : u))
			};
			break;
		case 'user_reconnected':
			state = {
				...state,
				users: state.users.map((u) => (u.user_id === sm.user_id ? { ...u, is_connected: true } : u))
			};
			break;
		case 'host_changed': {
			const becameHost = state.myId === sm.host_id;
			state = {
				...state,
				users: state.users.map((u) => ({ ...u, is_host: u.user_id === sm.host_id })),
				role: becameHost ? 'host' : 'guest',
				// The old (non-)host's pending joins/suggestions can no longer be acted on locally;
				// the server re-sends fresh ones to whoever is host now.
				...(becameHost ? {} : { pendingJoins: [], suggestions: [] })
			};
			if (becameHost) emitNotice("You're the host now.");
			break;
		}
		case 'sync_playback':
			// No local player on the web build yet (Phase 3) — just keep the display mirror current.
			if (sm.kind === 'change_track') {
				state = { ...state, currentTrack: sm.track ?? null, queue: sm.queue ?? state.queue };
			} else if (sm.kind === 'sync_queue' && sm.queue) {
				state = { ...state, queue: sm.queue };
			}
			break;
		case 'sync_state':
			absorb(sm.state);
			break;
		case 'suggestion_received':
			state = { ...state, suggestions: [...state.suggestions, sm.suggestion] };
			break;
		case 'suggestion_approved':
			state = { ...state, suggestions: state.suggestions.filter((s) => s.id !== sm.id) };
			emitNotice('Added to the session queue.');
			break;
		case 'suggestion_rejected':
			state = { ...state, suggestions: state.suggestions.filter((s) => s.id !== sm.id) };
			emitNotice(sm.reason);
			break;
		case 'kicked':
			closeLocally(sm.reason);
			return;
		case 'error': {
			emitNotice(sm.message);
			const inRoom = state.role !== 'none';
			if (!inRoom || sm.code === 'session_expired') {
				closeLocally('');
				return;
			}
			break;
		}
	}
	emitState();
}

// --- public API, matched 1:1 against the `lt*`/`onLt*` command surface --------------------

export function getState(): LtState {
	return state;
}

export function setServerUrl(url: string) {
	state = { ...state, serverUrl: url };
	emitState();
}

export function createRoom(username: string) {
	start({ type: 'create_room', username });
}

export function joinRoom(roomCode: string, username: string) {
	start({ type: 'join_room', room_code: roomCode, username });
}

export function leave() {
	send({ type: 'leave_room' });
	connectionGen += 1;
	teardownSocket();
	resetRoom();
	emitState();
}

export function approveJoin(userId: string) {
	send({ type: 'approve_join', user_id: userId });
}

export function rejectJoin(userId: string) {
	state = { ...state, pendingJoins: state.pendingJoins.filter((p) => p.userId !== userId) };
	send({ type: 'reject_join', user_id: userId });
	emitState();
}

export function kick(userId: string) {
	send({ type: 'kick_user', user_id: userId });
}

export function transferHost(userId: string) {
	send({ type: 'transfer_host', user_id: userId });
}

export function approveSuggestion(id: string) {
	state = { ...state, suggestions: state.suggestions.filter((s) => s.id !== id) };
	send({ type: 'approve_suggestion', id });
	emitState();
}

export function rejectSuggestion(id: string) {
	state = { ...state, suggestions: state.suggestions.filter((s) => s.id !== id) };
	send({ type: 'reject_suggestion', id });
	emitState();
}

export function requestSync() {
	send({ type: 'request_sync' });
}

/** Host-only: broadcast a play/pause/seek/track-change to the room (design.md D6 — the desktop
 *  equivalent is `LtSession::broadcast_playback`, called from `AppState`'s own playback-change
 *  points). Silently a no-op when not connected or not hosting — the caller (the web-playback
 *  store) doesn't need to check `state.role` itself before every transport action. */
export function broadcastPlayback(p: PlaybackWire) {
	if (state.role !== 'host') return;
	send({ type: 'playback', ...p });
}

export function onState(cb: StateListener): () => void {
	stateListeners.add(cb);
	return () => stateListeners.delete(cb);
}

export function onNotice(cb: NoticeListener): () => void {
	noticeListeners.add(cb);
	return () => noticeListeners.delete(cb);
}
