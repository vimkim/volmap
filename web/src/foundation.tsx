import { useEffect, useMemo, useReducer } from "react";
import { createRoot } from "react-dom/client";

import "../../src/web/assets/app.css";
import "../../src/web/assets/distribution.css";

import { createHttpApi } from "./api";
import { initialState, reduce } from "./model";
import { parseRoute } from "./routes";
import {
  createBrowserHistory,
  createRequestSignals,
  executeEffect,
  subscribeBrowserEvents,
  type RuntimePorts,
} from "./runtime";
import { Viewer } from "./view";

export function Application() {
  const requested = parseRoute(window.location.pathname) ?? { kind: "root" as const };
  const [state, dispatch] = useReducer(reduce, requested, initialState);
  const api = useMemo(() => createHttpApi(), []);
  const signals = useMemo(() => createRequestSignals(), []);
  const ports = useMemo<RuntimePorts>(() => ({
    api,
    history: createBrowserHistory(window),
    schedule: (milliseconds, action) => window.setTimeout(action, milliseconds),
    requestSignal: signals.signal,
  }), [api, signals]);

  useEffect(() => subscribeBrowserEvents(window, document, dispatch), []);
  useEffect(() => () => signals.abortAll(), [signals]);
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
