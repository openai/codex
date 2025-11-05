# AWS GPU クラスター見積もり

**目的**: Codex Cloud GPU クラスターの月次コスト試算  
**更新日**: 2025年11月2日  
**リージョン**: us-east-1 (N. Virginia)

---

## 📊 エグゼクティブサマリー

| 項目 | 値 |
|------|-----|
| **初期構成コスト** | $8,245/月 |
| **Year 1 平均コスト** | $12,500/月 ($150K/年) |
| **Year 2 スケール後** | $35,000/月 ($420K/年) |
| **予想収益** | Year 1: $666K, Year 2: $1.7M |
| **粗利益率** | Year 1: 77%, Year 2: 75% |

---

## 🏗️ アーキテクチャ構成

### Phase 1: MVP (Month 1-6)

```
[ ALB ]
   ↓
[ EKS Control Plane ]
   ↓
┌─────────────────────────────────────┐
│ Worker Nodes                        │
│ - 3x g5.xlarge (GPU instances)     │
│ - 2x t3.large (API servers)        │
│ - Auto-scaling: 3-10 GPU nodes     │
└─────────────────────────────────────┘
   ↓
[ RDS PostgreSQL ]
[ ElastiCache Redis ]
[ S3 (artifacts) ]
```

### Phase 2: Scale (Month 7-12)

```
[ CloudFront + ALB ]
   ↓
[ Multi-AZ EKS ]
   ↓
┌─────────────────────────────────────┐
│ Worker Nodes                        │
│ - 10x g5.xlarge (GPU instances)    │
│ - 5x t3.xlarge (API servers)       │
│ - Auto-scaling: 5-30 GPU nodes     │
└─────────────────────────────────────┘
   ↓
[ RDS Multi-AZ ]
[ ElastiCache Cluster ]
[ S3 + Glacier ]
```

---

## 💰 詳細コスト見積もり

### 1. Compute - EKS & EC2

#### 1.1 EKS Control Plane

```
EKS Cluster: $0.10/hour x 730 hours = $73/month
```

#### 1.2 GPU Worker Nodes (g5.xlarge)

**スペック**:
- GPU: 1x NVIDIA A10G (24GB VRAM)
- vCPU: 4
- RAM: 16 GB
- Storage: 250 GB NVMe SSD
- Network: Up to 10 Gbps
- **On-Demand**: $1.006/hour
- **Spot (70% discount)**: ~$0.30/hour

**推奨**: Spot Instancesをメインに、On-Demandをフォールバック

| 構成 | インスタンス数 | 時間/月 | コスト/月 |
|------|--------------|--------|----------|
| **Phase 1 (Spot 80%)** | 3 baseline | 730h | $657 |
| **Phase 1 (On-Demand 20%)** | - | - | $165 |
| **Phase 1 合計** | 3 avg | - | **$822** |
| | | |
| **Phase 2 (Spot 80%)** | 10 baseline | 730h | $2,190 |
| **Phase 2 (On-Demand 20%)** | - | - | $548 |
| **Phase 2 合計** | 10 avg | - | **$2,738** |

#### 1.3 API Server Nodes (t3.large)

```
t3.large: $0.0832/hour x 2 nodes x 730h = $121/month (Phase 1)
t3.xlarge: $0.1664/hour x 5 nodes x 730h = $607/month (Phase 2)
```

#### 1.4 Compute合計

| Phase | GPU Nodes | API Nodes | EKS | 合計 |
|-------|-----------|-----------|-----|------|
| Phase 1 | $822 | $121 | $73 | **$1,016** |
| Phase 2 | $2,738 | $607 | $73 | **$3,418** |

---

### 2. Database - RDS PostgreSQL

#### 2.1 Instance

```
# Phase 1
db.t3.medium: $0.068/hour x 730h = $50/month
Storage: 100GB SSD ($0.115/GB) = $12/month
Backup: 100GB ($0.095/GB) = $10/month
Phase 1 合計: $72/month

# Phase 2 (Multi-AZ)
db.r5.large: $0.24/hour x 2 (Multi-AZ) x 730h = $350/month
Storage: 500GB SSD = $58/month
Backup: 500GB = $48/month
Phase 2 合計: $456/month
```

---

### 3. Caching - ElastiCache Redis

#### 3.1 Instance

```
# Phase 1
cache.t3.micro: $0.017/hour x 730h = $12/month

# Phase 2 (Cluster mode)
cache.m5.large x 3 nodes: $0.176/hour x 3 x 730h = $385/month
```

---

### 4. Storage - S3

#### 4.1 S3 Standard

```
# Phase 1
Storage: 500 GB @ $0.023/GB = $12/month
PUT Requests: 100K @ $0.005/1K = $0.50/month
GET Requests: 1M @ $0.0004/1K = $0.40/month
Data Transfer Out: 100 GB @ $0.09/GB = $9/month
Phase 1 合計: $22/month

# Phase 2
Storage: 5 TB @ $0.023/GB = $115/month
PUT Requests: 1M @ $0.005/1K = $5/month
GET Requests: 10M @ $0.0004/1K = $4/month
Data Transfer Out: 1 TB @ $0.09/GB = $90/month
Phase 2 合計: $214/month
```

#### 4.2 S3 Glacier (Long-term backup)

```
# Phase 2 only
Storage: 2 TB @ $0.004/GB = $8/month
```

---

### 5. Network - ALB & CloudFront

#### 5.1 Application Load Balancer

```
# Phase 1 & 2
ALB: $0.0225/hour x 730h = $16/month
LCU (Load Balancer Capacity Units): $0.008/LCU x 730h x 10 LCU avg = $58/month
合計: $74/month
```

#### 5.2 CloudFront (Phase 2 only)

```
Data Transfer Out: 1 TB @ $0.085/GB = $85/month
HTTPS Requests: 10M @ $0.01/10K = $10/month
合計: $95/month
```

---

### 6. Monitoring & Logging

#### 6.1 CloudWatch

```
# Phase 1
Metrics: 50 custom @ $0.30 = $15/month
Logs: 10 GB @ $0.50/GB = $5/month
Alarms: 20 @ $0.10 = $2/month
Phase 1 合計: $22/month

# Phase 2
Metrics: 200 custom = $60/month
Logs: 100 GB = $50/month
Alarms: 50 = $5/month
Phase 2 合計: $115/month
```

#### 6.2 Container Insights

```
# Phase 2 only
EKS Container Insights: $0.30/container x 50 containers = $15/month
```

---

### 7. Security

#### 7.1 Secrets Manager

```
Secrets: 20 @ $0.40/secret = $8/month
API Calls: 100K @ $0.05/10K = $0.50/month
合計: $9/month
```

#### 7.2 Certificate Manager (ACM)

```
Public certificates: Free
Private CA: $400/month (Phase 2 only, optional)
```

#### 7.3 WAF (Web Application Firewall)

```
# Phase 2 only
Web ACL: $5/month
Rules: 10 @ $1/rule = $10/month
Requests: 10M @ $0.60/1M = $6/month
合計: $21/month
```

---

## 📊 月次コスト総計

### Phase 1 (Month 1-6)

| カテゴリ | コスト/月 |
|---------|----------|
| Compute (EKS + EC2) | $1,016 |
| Database (RDS) | $72 |
| Cache (Redis) | $12 |
| Storage (S3) | $22 |
| Network (ALB) | $74 |
| Monitoring (CloudWatch) | $22 |
| Security (Secrets Manager) | $9 |
| **Phase 1 合計** | **$1,227/month** |

### Phase 2 (Month 7-12)

| カテゴリ | コスト/月 |
|---------|----------|
| Compute (EKS + EC2) | $3,418 |
| Database (RDS Multi-AZ) | $456 |
| Cache (Redis Cluster) | $385 |
| Storage (S3 + Glacier) | $222 |
| Network (ALB + CloudFront) | $169 |
| Monitoring (CloudWatch + Insights) | $130 |
| Security (Secrets + WAF) | $30 |
| **Phase 2 合計** | **$4,810/month** |

---

## 🚀 スケーリング予測

### Year 1 コスト推移

| 月 | ユーザー数 | GPU Nodes | 月次コスト | 累積コスト |
|----|----------|-----------|----------|----------|
| 1 | 1,000 | 3 | $1,227 | $1,227 |
| 2 | 2,500 | 4 | $1,500 | $2,727 |
| 3 | 5,000 | 6 | $2,200 | $4,927 |
| 4 | 7,500 | 7 | $2,600 | $7,527 |
| 5 | 10,000 | 8 | $3,000 | $10,527 |
| 6 | 12,500 | 9 | $3,400 | $13,927 |
| 7 | 15,000 | 10 | $4,810 | $18,737 |
| 8 | 20,000 | 12 | $5,500 | $24,237 |
| 9 | 25,000 | 14 | $6,200 | $30,437 |
| 10 | 30,000 | 16 | $6,900 | $37,337 |
| 11 | 35,000 | 18 | $7,600 | $44,937 |
| 12 | 40,000 | 20 | $8,300 | **$53,237** |

**Year 1 平均**: $4,437/月 = **$53,237/年**

### Year 2 コスト推移 (加速成長)

| 四半期 | ユーザー数 | GPU Nodes | 月次平均コスト |
|--------|----------|-----------|--------------|
| Q1 | 50,000 | 25 | $10,500 |
| Q2 | 75,000 | 35 | $14,500 |
| Q3 | 100,000 | 50 | $20,000 |
| Q4 | 150,000 | 75 | $30,000 |

**Year 2 平均**: $18,750/月 = **$225,000/年**

---

## 💡 コスト最適化戦略

### 1. Spot Instances活用

```
Savings: 70% on GPU instances
Annual Savings: ~$15,000 (Year 1)

Implementation:
- 80% Spot, 20% On-Demand
- Spot interruption handling
- Multiple availability zones
```

### 2. Reserved Instances (1-year)

```
RDS Reserved: 40% off = $170/month savings (Year 2)
EC2 Reserved: 30% off = $800/month savings (Year 2)
Annual Savings: ~$11,640 (Year 2)
```

### 3. S3 Intelligent-Tiering

```
Automatic cost optimization
Savings: 30-70% on infrequently accessed data
Annual Savings: ~$500 (Year 2)
```

### 4. Auto-scaling最適化

```
Min nodes: 3 (Phase 1), 5 (Phase 2)
Max nodes: 10 (Phase 1), 30 (Phase 2)
Target utilization: 70%
Cost reduction: 20-30% during off-peak
```

### 5. Multi-region避ける（初期）

```
Savings: No cross-region transfer fees
Cost: Single region only (us-east-1)
```

---

## 📈 ROI分析

### Phase 1 (Month 1-6)

```
Revenue (Month 6):
  - Free: 10,000 users x $0 = $0
  - Pro: 500 users x $15 = $7,500/month
  - Team: 30 teams x $50 = $1,500/month
  - Enterprise: 2 companies x $500 = $1,000/month
  Total: $10,000/month

Infrastructure Cost: $3,400/month (Month 6)

Gross Margin: 66%
```

### Year 1 (Full Year)

```
Total Revenue: $666,980
Total Infrastructure Cost: $53,237
Other Costs (staff, marketing, office): ~$200,000
Net Profit: $413,743

ROI: 207%
```

### Year 2 (Projected)

```
Total Revenue: $1,700,000
Total Infrastructure Cost: $225,000
Other Costs: ~$500,000
Net Profit: $975,000

ROI: 144%
```

---

## ⚠️ リスクと対策

### 1. Spot Instance中断

**リスク**: GPU Spot中断で処理失敗

**対策**:
- 80% Spot + 20% On-Demand混在
- Graceful shutdown (2分前通知)
- チェックポイント保存
- 自動リトライ

### 2. GPU不足

**リスク**: g5.xlarge不足でスケール不可

**対策**:
- 複数リージョン待機（us-east-1, us-west-2）
- 代替インスタンス（g4dn.xlarge）準備
- クォータ事前増加申請

### 3. コスト超過

**リスク**: 予想外のトラフィック増

**対策**:
- CloudWatch billing alerts
- Auto-scaling上限設定
- Rate limiting実装
- コスト可視化ダッシュボード

---

## 🔄 代替案

### Option A: AWS SageMaker

```
Pros:
  - Managed ML infrastructure
  - Auto-scaling built-in
  - Simplified deployment

Cons:
  - Higher cost (30-50% more)
  - Less flexibility
  - Vendor lock-in

Cost: ~$7,000/month (Phase 1)
```

### Option B: GCP + NVIDIA

```
Pros:
  - Better GPU pricing (10-15% cheaper)
  - Preemptible TPU access
  - GKE優れている

Cons:
  - Less mature ecosystem
  - Migration effort

Cost: ~$1,100/month (Phase 1)
```

### Option C: Hybrid (AWS + Lambda)

```
Pros:
  - Pay-per-use
  - No idle cost
  - Infinite scale

Cons:
  - Cold start latency
  - 15min timeout limit
  - Complex architecture

Cost: Variable, ~$2,000-5,000/month
```

**推奨**: AWS EKS + Spot Instances（最もバランスが良い）

---

## 📋 実装チェックリスト

### Infrastructure as Code

- [ ] Terraform モジュール作成
- [ ] EKS cluster definition
- [ ] RDS/Redis setup
- [ ] S3 bucket configuration
- [ ] IAM roles & policies

### CI/CD

- [ ] GitHub Actions pipeline
- [ ] Docker image build
- [ ] Kubernetes deployment
- [ ] Helm charts
- [ ] Auto-rollback setup

### Monitoring

- [ ] CloudWatch dashboards
- [ ] Prometheus + Grafana
- [ ] PagerDuty alerts
- [ ] Cost anomaly detection

### Security

- [ ] VPC & subnet isolation
- [ ] Security groups
- [ ] KMS encryption
- [ ] Secrets rotation
- [ ] WAF rules

---

## 📚 参考資料

- [AWS Pricing Calculator](https://calculator.aws/)
- [EKS Best Practices](https://aws.github.io/aws-eks-best-practices/)
- [GPU Spot Instance Advisor](https://aws.amazon.com/ec2/spot/instance-advisor/)

---

**次のステップ**: 
1. Terraformでインフラ定義
2. Dev環境構築（Small scale）
3. Load testing
4. Production deployment

**見積もり有効期限**: 2025年12月31日（価格変動の可能性あり）

