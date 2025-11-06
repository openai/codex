/**
 * Plan state management for VS Code extension
 */

export enum PlanState {
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

export interface PlanBlock {
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
    state: PlanState;
    needApproval: boolean;
    createdAt: string;
    updatedAt: string;
    createdBy?: string;
}

export class PlanStateManager {
    private currentPlan: PlanBlock | null = null;
    private _isPlanModeActive = false;
    
    /**
     * Get current Plan
     */
    getCurrentPlan(): PlanBlock | null {
        return this.currentPlan;
    }
    
    /**
     * Set current Plan
     */
    setCurrentPlan(Plan: PlanBlock | null): void {
        this.currentPlan = Plan;
    }
    
    /**
     * Check if plan mode is active
     */
    isPlanModeActive(): boolean {
        return this._isPlanModeActive;
    }
    
    /**
     * Enable plan mode
     */
    enablePlanMode(): void {
        this._isPlanModeActive = true;
    }
    
    /**
     * Disable plan mode
     */
    disablePlanMode(): void {
        this._isPlanModeActive = false;
        this.currentPlan = null;
    }
    
    /**
     * Check if Plan can be executed
     */
    canExecute(): boolean {
        return this.currentPlan?.state === PlanState.Approved;
    }
    
    /**
     * Check if Plan can be modified
     */
    canModify(): boolean {
        if (!this.currentPlan) {
            return false;
        }
        
        const state = this.currentPlan.state;
        return state === PlanState.Inactive || state === PlanState.Drafting;
    }
    
    /**
     * Get state color for UI
     */
    getStateColor(state: PlanState): string {
        switch (state) {
            case PlanState.Pending:
                return 'orange';
            case PlanState.Approved:
                return 'green';
            case PlanState.Rejected:
                return 'red';
            case PlanState.Superseded:
                return 'gray';
            case PlanState.Drafting:
                return 'blue';
            default:
                return 'gray';
        }
    }
    
    /**
     * Get state icon for UI
     */
    getStateIcon(state: PlanState): string {
        switch (state) {
            case PlanState.Pending:
                return '⏱️';
            case PlanState.Approved:
                return '✅';
            case PlanState.Rejected:
                return '❌';
            case PlanState.Superseded:
                return '🔄';
            case PlanState.Drafting:
                return '✏️';
            default:
                return '📋';
        }
    }
}

