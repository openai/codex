from __future__ import annotations
from dataclasses import dataclass
from typing import List, Optional, Dict
import math
import numpy as np
from .utils import log

try:
    import pennylane as qml
except Exception:
    qml = None  # type: ignore

@dataclass
class QMLConfig:
    backend: str = "default.qubit"
    n_qubits: int = 4
    layers: int = 2
    lr: float = 0.08
    epochs: int = 25

class QuantumMLHead:
    def __init__(self, cfg: QMLConfig):
        self.cfg = cfg
        self.enabled = qml is not None
        self._X: List[np.ndarray] = []
        self._y: List[float] = []
        self._weights: Optional[np.ndarray] = None
        if not self.enabled:
            log.warning("PennyLane not installed -> QML disabled (pip install pennylane).")
            return
        self.dev = qml.device(cfg.backend, wires=cfg.n_qubits)

        @qml.qnode(self.dev)
        def circuit(x, w):
            for i in range(cfg.n_qubits):
                qml.RY(x[i], wires=i)
            idx = 0
            for _ in range(cfg.layers):
                for i in range(cfg.n_qubits):
                    qml.RY(w[idx], wires=i); idx += 1
                    qml.RZ(w[idx], wires=i); idx += 1
                for i in range(cfg.n_qubits-1):
                    qml.CNOT(wires=[i,i+1])
                qml.CNOT(wires=[cfg.n_qubits-1,0])
            return qml.expval(qml.PauliZ(0))
        self.circuit = circuit
        n_params = 2*cfg.n_qubits*cfg.layers
        self._weights = 0.01*np.random.randn(n_params).astype(np.float64)

    def add_example(self, x: np.ndarray, y: float) -> None:
        if not self.enabled or self._weights is None: return
        if x.shape[0] != self.cfg.n_qubits: return
        self._X.append(x.astype(np.float64))
        self._y.append(1.0 if y >= 0 else -1.0)
        if len(self._X) > 600:
            self._X = self._X[-600:]
            self._y = self._y[-600:]

    def ready(self) -> bool:
        return bool(self.enabled and self._weights is not None and len(self._X) >= 60)

    def train(self) -> Dict[str, float]:
        if not self.ready():
            return {"trained": 0.0, "n": float(len(self._X))}
        X = np.stack(self._X, axis=0)
        y = np.array(self._y, dtype=np.float64)
        w = self._weights.copy()
        opt = qml.GradientDescentOptimizer(self.cfg.lr)
        def loss(ww):
            preds = np.array([self.circuit(xx, ww) for xx in X], dtype=np.float64)
            margins = 1.0 - (y*preds)
            return float(np.mean(np.maximum(0.0, margins)))
        for _ in range(int(self.cfg.epochs)):
            w = opt.step(loss, w)
        self._weights = w
        return {"trained": 1.0, "n": float(len(self._X))}

    def predict(self, x: np.ndarray) -> float:
        if not self.enabled or self._weights is None: return 0.0
        if x.shape[0] != self.cfg.n_qubits: return 0.0
        xx = np.array([math.tanh(float(v))* (math.pi/2) for v in x], dtype=np.float64)
        return float(max(-1.0, min(1.0, self.circuit(xx, self._weights))))
