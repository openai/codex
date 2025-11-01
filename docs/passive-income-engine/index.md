# Passive Income Engine — System Map

The Passive Income Engine coordinates data ingestion, strategy orchestration, and automated execution across multiple financial venues. The system map below highlights the major subsystems and how they exchange information to deliver resilient, compliant yield strategies.

```mermaid
%%{init: {'theme': 'neutral', 'flowchart': {'curve': 'basis'}}}%%
%% Passive Income Engine — System Map
flowchart TD
    subgraph Inputs["Investor Inputs"]
        goals["Investor Goals & Risk Appetite"]
        compliance["Compliance Constraints"]
    end

    subgraph DataLayer["Data Acquisition Layer"]
        marketFeeds["Market Data Connectors\n(CEX/DEX, Aggregators)"]
        defiOracles["DeFi Oracles & Yield Indexers"]
        chainData["On-chain Indexers"]
        altSignals["Alternative Data Signals"]
    end

    subgraph Core["Strategy Orchestration Core"]
        strategyLab["Strategy Lab"]
        riskEngine["Risk & Guardrails Engine"]
        scheduler["Automation Scheduler"]
    end

    subgraph Execution["Execution Layer"]
        walletManager["Wallet & Account Manager"]
        smartContracts["Smart Contract Executors"]
        exchangeAPIs["Exchange & Broker APIs"]
    end

    subgraph Feedback["Monitoring & Analytics"]
        telemetry["Telemetry Pipeline"]
        reports["Investor Dashboard"]
        alerts["Alerting Service"]
    end

    goals --> strategyLab
    compliance --> riskEngine

    marketFeeds --> strategyLab
    defiOracles --> strategyLab
    chainData --> strategyLab
    altSignals --> strategyLab

    strategyLab --> riskEngine
    riskEngine --> scheduler
    scheduler --> walletManager

    walletManager --> smartContracts
    walletManager --> exchangeAPIs

    smartContracts --> telemetry
    exchangeAPIs --> telemetry

    telemetry --> reports
    telemetry --> alerts
    telemetry --> strategyLab

    alerts --> goals
    reports --> goals
```

## Editing the diagram

- Update the source diagram in `docs/passive-income-engine/Passive_Income_Engine_System_Map.mmd`.
- Optional: regenerate a static asset for PDFs or slide decks with the Mermaid CLI.

```bash
npm i -g @mermaid-js/mermaid-cli
mmdc -i docs/passive-income-engine/Passive_Income_Engine_System_Map.mmd \
     -o docs/passive-income-engine/Passive_Income_Engine_System_Map.svg \
     -b transparent
```

> MkDocs users: make sure the `mermaid2` plugin is enabled alongside `pymdownx.superfences` and `pymdownx.tabbed` in `mkdocs.yml` so the diagram renders during documentation builds.
