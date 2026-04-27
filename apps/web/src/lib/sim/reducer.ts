import type { ServerMessage } from "@/types/sim/ServerMessage";
import type { Snapshot } from "@/types/sim/Snapshot";

export type SimStatus = "connecting" | "connected" | "disconnected";

export type SimState = {
  status: SimStatus;
  snapshot: Snapshot | null;
  lastTick: number | null;
};

export const initialState: SimState = {
  status: "connecting",
  snapshot: null,
  lastTick: null,
};

export type Action =
  | { kind: "server-message"; msg: ServerMessage }
  | { kind: "ws-open" }
  | { kind: "ws-close" }
  | { kind: "ws-error" };

export function reduce(state: SimState, action: Action): SimState {
  switch (action.kind) {
    case "ws-open":
      return state.status === "disconnected" ? state : { ...state, status: "connecting" };
    case "ws-close":
    case "ws-error":
      return { ...state, status: "disconnected" };
    case "server-message":
      return reduceServerMessage(state, action.msg);
  }
}

function reduceServerMessage(state: SimState, msg: ServerMessage): SimState {
  switch (msg.type) {
    case "hello":
      // Handshake greeting; transport layer responds with ClientHello.
      // No state change — still "connecting" until Init lands.
      return state;
    case "init":
      return {
        status: "connected",
        snapshot: msg.snapshot,
        lastTick: msg.snapshot.tick,
      };
    case "snapshot":
      return {
        ...state,
        snapshot: msg.snapshot,
        lastTick: msg.snapshot.tick,
      };
  }
}
