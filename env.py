from __future__ import annotations
import numpy as np
import gymnasium as gym
from gymnasium import spaces

class TradingEnv(gym.Env):
    def __init__(self):
        super().__init__()
        self.action_space = spaces.Discrete(3)
        self.observation_space = spaces.Box(low=-np.inf, high=np.inf, shape=(8,), dtype=np.float32)
        self.state = np.zeros(8, dtype=np.float32)

    def reset(self, seed=None, options=None):
        super().reset(seed=seed)
        self.state = np.random.randn(8).astype(np.float32) * 0.1
        return self.state, {}

    def step(self, action):
        reward = float(np.random.randn() * 0.01)
        term = bool(np.random.rand() < 0.02)
        return self.state, reward, term, False, {}
