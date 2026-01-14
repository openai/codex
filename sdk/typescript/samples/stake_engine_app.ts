#!/usr/bin/env -S NODE_NO_WARNINGS=1 pnpm ts-node-esm --files

import crypto from "node:crypto";

interface Player {
  id: string;
  name: string;
  balance: number;
  locked: number;
  history: string[];
}

interface MatchPlayer {
  playerId: string;
  score: number;
  reward: number;
  penalized: boolean;
}

type MatchState = "pending" | "active" | "settled";

interface MatchConfig {
  entryFee: number;
  rewardPoolPercent: number;
  slashPercent: number;
  rounds: number;
  rngSeed: string;
}

interface Match {
  id: string;
  state: MatchState;
  config: MatchConfig;
  players: MatchPlayer[];
}

class SeededRng {
  private state: bigint;

  constructor(seed: string) {
    const hash = crypto.createHash("sha256").update(seed).digest();
    this.state = BigInt("0x" + hash.toString("hex"));
  }

  next(): number {
    // xorshift64*
    this.state ^= this.state << 13n;
    this.state ^= this.state >> 7n;
    this.state ^= this.state << 17n;
    const result = Number(this.state & ((1n << 53n) - 1n));
    return result / Number((1n << 53n) - 1n);
  }
}

class StakeEngine {
  private players = new Map<string, Player>();
  private matches: Match[] = [];

  registerPlayer(name: string): Player {
    const id = crypto.randomUUID();
    const player: Player = { id, name, balance: 0, locked: 0, history: [] };
    this.players.set(id, player);
    player.history.push(`Player ${name} registered with id ${id}`);
    return player;
  }

  topUp(playerId: string, amount: number): void {
    const player = this.requirePlayer(playerId);
    player.balance += amount;
    player.history.push(`Topped up ${amount.toFixed(2)} tokens`);
  }

  createMatch(config: MatchConfig): Match {
    const match: Match = {
      id: crypto.randomUUID(),
      state: "pending",
      config,
      players: [],
    };
    this.matches.push(match);
    return match;
  }

  joinMatch(matchId: string, playerId: string): void {
    const match = this.requireMatch(matchId);
    const player = this.requirePlayer(playerId);
    if (match.state !== "pending") {
      throw new Error("Cannot join a match that has started");
    }
    if (player.balance < match.config.entryFee) {
      throw new Error(`Insufficient balance for ${player.name}`);
    }
    player.balance -= match.config.entryFee;
    player.locked += match.config.entryFee;
    player.history.push(`Entered match ${matchId} with fee ${match.config.entryFee.toFixed(2)}`);
    match.players.push({ playerId, score: 0, reward: 0, penalized: false });
  }

  startMatch(matchId: string): void {
    const match = this.requireMatch(matchId);
    if (match.state !== "pending") {
      throw new Error("Match already started");
    }
    if (match.players.length === 0) {
      throw new Error("Cannot start an empty match");
    }
    match.state = "active";
  }

  settleMatch(matchId: string): void {
    const match = this.requireMatch(matchId);
    if (match.state !== "active") {
      throw new Error("Match is not active");
    }

    const rng = new SeededRng(match.config.rngSeed);
    for (const player of match.players) {
      player.score = this.playRounds(match.config.rounds, rng);
      player.penalized = rng.next() < match.config.slashPercent / 100;
    }

    const sorted = [...match.players].sort((a, b) => b.score - a.score);
    const pool = match.players.length * match.config.entryFee * (match.config.rewardPoolPercent / 100);
    const winner = sorted[0];
    winner.reward = Number(pool.toFixed(2));
    const penalty = match.config.entryFee * (match.config.slashPercent / 100);

    for (const player of match.players) {
      const account = this.requirePlayer(player.playerId);

      // release the previously locked entry fee
      account.locked -= match.config.entryFee;

      if (player === winner) {
        // award winner: refund stake + reward
        account.balance += match.config.entryFee + winner.reward;
        account.history.push(`Won match ${match.id} and earned ${winner.reward.toFixed(2)}`);
      } else {
        // losers currently do not receive a refund of their stake in this demo (stakes contribute to pool)
        // keep behavior: locked was removed above; record entry for losers
        account.history.push(`Lost match ${match.id}`);
      }

      // apply penalty if flagged
      if (player.penalized) {
        const penaltyAmt = Number(penalty.toFixed(2));
        // deduct penalty from available balance (clamp to zero)
        const deduct = Math.min(account.balance, penaltyAmt);
        account.balance -= deduct;
        account.history.push(`Penalized ${deduct.toFixed(2)} in match ${match.id}`);
      }
    }

    match.state = "settled";
  }

  withdraw(playerId: string, amount: number): void {
    const player = this.requirePlayer(playerId);
    if (amount > player.balance) {
      throw new Error(`Cannot withdraw ${amount}; available ${player.balance.toFixed(2)}`);
    }
    player.balance -= amount;
    player.history.push(`Withdrew ${amount.toFixed(2)} tokens`);
  }

  summary(): Player[] {
    return [...this.players.values()];
  }

  private playRounds(rounds: number, rng: SeededRng): number {
    let score = 0;
    for (let i = 0; i < rounds; i += 1) {
      score += Math.round(rng.next() * 100);
    }
    return score;
  }

  private requirePlayer(id: string): Player {
    const player = this.players.get(id);
    if (!player) {
      throw new Error(`Unknown player ${id}`);
    }
    return player;
  }

  private requireMatch(id: string): Match {
    const match = this.matches.find((m) => m.id === id);
    if (!match) {
      throw new Error(`Unknown match ${id}`);
    }
    return match;
  }
}

const printLedger = (players: Player[]): void => {
  console.log("\n=== Player Balances ===");
  for (const player of players) {
    console.log(`${player.name}: available=${player.balance.toFixed(2)}, locked=${player.locked.toFixed(2)}`);
  }
};

const printHistory = (players: Player[]): void => {
  console.log("\n=== Audit Trail ===");
  for (const player of players) {
    console.log(`\n${player.name}`);
    for (const entry of player.history) {
      console.log(`  - ${entry}`);
    }
  }
};

const demo = (): void => {
  const engine = new StakeEngine();
  const ava = engine.registerPlayer("Ava");
  const noor = engine.registerPlayer("Noor");
  const sol = engine.registerPlayer("Sol");

  engine.topUp(ava.id, 120);
  engine.topUp(noor.id, 75);
  engine.topUp(sol.id, 60);

  const match = engine.createMatch({
    entryFee: 25,
    rewardPoolPercent: 70,
    slashPercent: 20,
    rounds: 5,
    rngSeed: "stake-engine-demo",
  });

  engine.joinMatch(match.id, ava.id);
  engine.joinMatch(match.id, noor.id);
  engine.joinMatch(match.id, sol.id);

  engine.startMatch(match.id);
  engine.settleMatch(match.id);

  engine.withdraw(ava.id, 20);

  const players = engine.summary();
  printLedger(players);
  printHistory(players);
};

demo();
