import { useEffect, useMemo, useReducer, useRef } from "react";
import { createRoot } from "react-dom/client";

import "../../src/web/assets/app.css";
import "../../src/web/assets/distribution.css";
import "./observations.css";

import { createHttpApi } from "./api";
import { initialState, reduce } from "./model";
import { parseRoute } from "./routes";
import {
  createBrowserHistory,
  createRequestSignals,
  executeEffect,
  subscribeBrowserEvents,
  subscribeObservationExpiry,
  type RuntimePorts,
} from "./runtime";
import { Viewer } from "./view";

export function Application() {
  const requested = parseRoute(window.location.pathname) ?? { kind: "root" as const };
  const [state, dispatch] = useReducer(reduce, requested, initialState);
  const current = useRef(state);
  current.current = state;
  const api = useMemo(() => createHttpApi(), []);
  const signals = useMemo(() => createRequestSignals(), []);
  const ports = useMemo<RuntimePorts>(() => ({
    api,
    allowRuntime: (effect) => {
      const latest = current.current;
      if (document.visibilityState !== "visible" || !latest.visible) return false;
      if (effect.kind === "read-runtime-capability") return effect.epoch === undefined || (latest.observation.enabled && effect.epoch === latest.observation.epoch);
      return latest.observation.enabled && !latest.follow.paused && effect.scope === latest.scope && effect.request.epoch === String(latest.observation.epoch);
    },
    history: createBrowserHistory(window),
    schedule: (milliseconds, action) => window.setTimeout(action, milliseconds),
    requestSignal: signals.signal,
  }), [api, signals]);

  useEffect(() => subscribeBrowserEvents(window, document, dispatch), []);
  useEffect(() => subscribeObservationExpiry(window, state.observation, dispatch),
    [state.observation.batch, state.observation.age, state.observation.received, state.observation.wallReceived]);
  useEffect(() => () => signals.abortAll(), [signals]);
  useEffect(() => { signals.abortRuntime(); }, [signals, state.observation.epoch, state.visible, state.follow.paused]);
  useEffect(() => {
    if (state.effects.length === 0) return;
    const effects = state.effects;
    dispatch({ kind: "effects-started", ids: effects.map((effect) => effect.id) });
    for (const effect of effects) void executeEffect(effect, ports, dispatch);
  }, [ports, state.effects]);

  return <Viewer state={state} dispatch={dispatch} nowUnixSeconds={state.nowUnixSeconds} />;
}

const host = typeof document === "undefined" ? null : document.getElementById("volmap-react-root");
if (host !== null) {
  host.dataset.viewer = "react";
  createRoot(host).render(<Application />);
}
