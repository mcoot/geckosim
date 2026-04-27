"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useReducer,
  useRef,
  type ReactNode,
} from "react";
import type { ClientMessage } from "@/types/sim/ClientMessage";
import type { PlayerInput } from "@/types/sim/PlayerInput";
import type { ServerMessage } from "@/types/sim/ServerMessage";
import { initialState, reduce, type SimState } from "./reducer";

const DEFAULT_WS_URL = "ws://127.0.0.1:9001/";
const WS_URL = process.env.NEXT_PUBLIC_SIM_WS_URL ?? DEFAULT_WS_URL;

// Mirrors `gecko_sim_protocol::PROTOCOL_VERSION`. Bumped in lock-step on
// incompatible wire changes; `ts-rs` does not auto-export bare consts so
// this is hand-maintained. A mismatch logs a warning but does not refuse
// the connection at v0.
const EXPECTED_PROTOCOL_VERSION = 1;

export interface SimConnectionApi {
  state: SimState;
  sendInput: (input: PlayerInput) => void;
}

const SimConnectionContext = createContext<SimConnectionApi | null>(null);

export function useSimConnection(): SimConnectionApi {
  const ctx = useContext(SimConnectionContext);
  if (!ctx) {
    throw new Error("useSimConnection must be used inside <SimConnectionProvider>");
  }
  return ctx;
}

export function SimConnectionProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(reduce, initialState);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    const ws = new WebSocket(WS_URL);
    wsRef.current = ws;

    ws.onopen = () => dispatch({ kind: "ws-open" });
    ws.onclose = () => dispatch({ kind: "ws-close" });
    ws.onerror = () => dispatch({ kind: "ws-error" });
    ws.onmessage = (event) => {
      let msg: ServerMessage;
      try {
        msg = JSON.parse(event.data) as ServerMessage;
      } catch {
        console.warn("dropping non-JSON frame", event.data);
        return;
      }
      dispatch({ kind: "server-message", msg });

      // Hello → check version, reply with ClientHello.
      if (msg.type === "hello") {
        if (msg.protocol_version !== EXPECTED_PROTOCOL_VERSION) {
          console.warn(
            `protocol version mismatch: server=${msg.protocol_version}, ` +
              `client=${EXPECTED_PROTOCOL_VERSION}; proceeding anyway at v0`,
          );
        }
        const reply: ClientMessage = { type: "client_hello", last_known_tick: null };
        ws.send(JSON.stringify(reply));
      }
    };

    return () => {
      ws.close();
      wsRef.current = null;
    };
  }, []);

  const sendInput = useCallback((input: PlayerInput) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      console.warn("sendInput called while WS not open; ignoring", input);
      return;
    }
    // ClientMessage::PlayerInput(PlayerInput) — serde flattens the inner enum's
    // tag into the outer object via #[serde(tag = "type")] on ClientMessage and
    // #[serde(tag = "kind")] on PlayerInput, so the wire shape is:
    //   { "type": "player_input", "kind": "set_speed", "multiplier": 2.0 }
    const msg = { type: "player_input" as const, ...input };
    ws.send(JSON.stringify(msg));
  }, []);

  return (
    <SimConnectionContext.Provider value={{ state, sendInput }}>
      {children}
    </SimConnectionContext.Provider>
  );
}
