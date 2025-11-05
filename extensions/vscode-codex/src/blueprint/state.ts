/**
 * Blueprint state management for VS Code extension
 */

export enum BlueprintState {
    Inactive = 'inactive',
    Drafting = 'drafting',
    Pending = 'pending',
    Approved = 'approved',
    Rejected = 'rejected',
    Superseded = 'superseded'
}

export enum ExecutionMode {
    Single = 'single',
    Orchestrated = 'orchestrated',
    Competition = 'competition'
}

export interface WorkItem {
    name: string;
    filesTouched: string[];
    diffContract: string;
    tests: string[];
}

export interface Risk {
    item: string;
    mitigation: string;
}

export interface EvalCriteria {
    tests: string[];
    metrics: Record<string, string>;
}

export interface Budget {
    maxStep?: number;
    sessionCap?: number;
    estimateMin?: number;
    capMin?: number;
}

export interface ResearchSource {
    title: string;
    url: string;
    date: string;
    keyFinding: string;
    confidence: number;
}

export interface ResearchBlock {
    query: string;
    depth: number;
    strategy: string;
    sources: ResearchSource[];
    synthesis: string;
    confidence: number;
    needsApproval: boolean;
    timestamp: string;
}

export interface BlueprintBlock {
    id: string;
    title: string;
    goal: string;
    assumptions: string[];
    clarifyingQuestions: string[];
    approach: string;
    mode: ExecutionMode;
    workItems: WorkItem[];
    risks: Risk[];
    eval: EvalCriteria;
    budget: Budget;
    rollback: string;
    artifacts: string[];
    research?: ResearchBlock;
    state: BlueprintState;
    needApproval: boolean;
    createdAt: string;
    updatedAt: string;
    createdBy?: string;
}

export class BlueprintStateManager {
    private currentBlueprint: BlueprintBlock | null = null;
    private _isBlueprintModeActive = false;
    
    /**
     * Get current blueprint
     */
    getCurrentBlueprint(): BlueprintBlock | null {
        return this.currentBlueprint;
    }
    
    /**
     * Set current blueprint
     */
    setCurrentBlueprint(blueprint: BlueprintBlock | null): void {
        this.currentBlueprint = blueprint;
    }
    
    /**
     * Check if blueprint mode is active
     */
    isBlueprintModeActive(): boolean {
        return this._isBlueprintModeActive;
    }
    
    /**
     * Enable blueprint mode
     */
    enableBlueprintMode(): void {
        this._isBlueprintModeActive = true;
    }
    
    /**
     * Disable blueprint mode
     */
    disableBlueprintMode(): void {
        this._isBlueprintModeActive = false;
        this.currentBlueprint = null;
    }
    
    /**
     * Check if blueprint can be executed
     */
    canExecute(): boolean {
        return this.currentBlueprint?.state === BlueprintState.Approved;
    }
    
    /**
     * Check if blueprint can be modified
     */
    canModify(): boolean {
        if (!this.currentBlueprint) {
            return false;
        }
        
        const state = this.currentBlueprint.state;
        return state === BlueprintState.Inactive || state === BlueprintState.Drafting;
    }
    
    /**
     * Get state color for UI
     */
    getStateColor(state: BlueprintState): string {
        switch (state) {
            case BlueprintState.Pending:
                return 'orange';
            case BlueprintState.Approved:
                return 'green';
            case BlueprintState.Rejected:
                return 'red';
            case BlueprintState.Superseded:
                return 'gray';
            case BlueprintState.Drafting:
                return 'blue';
            default:
                return 'gray';
        }
    }
    
    /**
     * Get state icon for UI
     */
    getStateIcon(state: BlueprintState): string {
        switch (state) {
            case BlueprintState.Pending:
                return '⏱️';
            case BlueprintState.Approved:
                return '✅';
            case BlueprintState.Rejected:
                return '❌';
            case BlueprintState.Superseded:
                return '🔄';
            case BlueprintState.Drafting:
                return '✏️';
            default:
                return '📋';
        }
    }
}

