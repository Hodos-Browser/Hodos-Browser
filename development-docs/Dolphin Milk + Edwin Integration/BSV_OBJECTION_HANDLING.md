# BSV Objection Handling — Pitch Talking Points

**For:** Build AI on AWS Golden Pitch Competition (June 25) + Futran/Beck/SBDC audience + any future pitch where BSV will be questioned
**Prepared:** 2026-05-30
**Why this doc exists:** Both Futran and Beck have zero crypto/blockchain exposure. Mainstream tech has reputation baggage around "Bitcoin SV" — Craig Wright, niche markets, courtroom drama. We do NOT lead with BSV. But Matt is a hard line: we will not pretend BSV isn't the foundation. When the question comes, we need a ready, calibrated answer. This is that answer.

---

## The default posture

1. **Don't lead with it.** First mention of "Bitcoin SV" in the deck or pitch should not come from us.
2. **Reframe to value, not chain.** *"Cryptographic payment infrastructure," "machine-readable micropayment rails for AI agents," "sub-cent payment protocol," "programmable consent layer."* The chain is implementation detail.
3. **When asked: AWS-first answer.** Lead the defense with the AWS partnership, not BSV history.
4. **Don't engage the Craig Wright thread.** If pressed, redirect to engineering. AWS itself chose not to engage with it; we can do the same.

---

## The 30-second stock answer (memorize verbatim)

> *"AWS published a case study on their official Web3 blog in March 2026: the BSV Association sustained one million transactions per second for two weeks across six AWS regions, using EKS, FSx for Lustre, and RDS. That's roughly fifteen times Visa's peak throughput, sustained — not a benchmark spike. Whatever you've heard about the personalities, the engineering and the AWS partnership are real and current. The reason we chose BSV is the same reason AWS chose to feature it: it's the only public chain where per-prompt micropayments are economically viable."*

Three things this answer does:
1. **Cites AWS as the source** — not us, not BSV advocates, AWS
2. **Quantifies in a way they'll respect** — Visa is the right comparison for non-technical audiences
3. **Refuses to engage controversy** without being dismissive about it

---

## The article in question

| Field | Value |
|---|---|
| URL | https://aws.amazon.com/blogs/web3/how-the-bsv-association-built-a-million-tps-blockchain-node-using-aws/ |
| Published | March 31, 2026 |
| Publisher | AWS (official Web3 blog) |
| Author | Jordan Kramsky, Senior Solutions Architect for Startups, AWS |
| BSV side quoted | Siggi Óskarsson, CTO of the BSV Association |
| Headline claim | **1.06M TPS sustained for 2 weeks across 6 AWS regions** |

### AWS services used (talking points for AWS-savvy audiences)
EC2 (NVMe storage-optimized), **EKS** (Kubernetes), **FSx for Lustre**, S3, **RDS PostgreSQL**, VPC, Transit Gateway, MSK, Amazon Managed Service for Prometheus, Amazon Managed Grafana.

This is a deep, multi-service AWS architecture — not a single VM. AWS itself made the architectural choices in their published guide.

### Verbatim quote: TPS claim
> *"1 million consistent, zero-loss, TPS for at least 2 weeks (roughly one difficulty epoch) across six distributed AWS Regions"*

Six regions: `us-east-1, us-west-2, eu-west-1, ap-south-1, ap-northeast-2, ca-central-1`.

Previous baseline (SVNode): 13,614 peak TPS → **66.67× improvement.**

### Verbatim quote: Teranode (the software)
> *"A new reference node software"* whose innovation is *"continuous propagation of Merkle subtrees, enabling complex, verifiable cryptographic structures to pre-assemble among nodes."*

### Verbatim quote: BSV Association (the organization)
> *"Serves as the BSV network's coordinating body. They drive technical standards, educate developers, engage regulators, and foster commercial adoption worldwide."*

**Use this framing when the BSV Association comes up:** *standards body that engages regulators.* That's how AWS chose to describe them. We can reuse that exact framing.

### Verbatim quote: Óskarsson (memorize)
> *"It would have been almost impossible to build and improve Teranode in a timely manner otherwise... AWS has really helped us get this done so quickly."*

### Crucial omission
**AWS does not mention Craig Wright anywhere in the article.** AWS framed BSV purely as a technical scaling story and chose to ignore the controversy. That's strategic guidance for us: AWS-the-publisher set the precedent that we can follow.

---

## Comparison talking points (external, you cite — not from the article)

| Network | TPS (peak / sustained) | Notes |
|---|---|---|
| Visa | ~65,000 peak | Their published max |
| Bitcoin (BTC) | ~7 sustained | Block-time + size constraint |
| Ethereum (ETH) | ~30 sustained | Even with L2s, base is constrained |
| Solana | ~50,000 peak (theoretical) | Real-world has been lower; outages |
| **BSV (via Teranode on AWS)** | **1,060,000 sustained, 14 days** | AWS-published, March 2026 |

**Slide-able line:** *"15× sustained Visa throughput, on AWS, published by AWS. The math problem is solved; we use it because nobody else has solved it."*

---

## Likely objections + responses

### "Isn't BSV the Craig Wright thing?"

> *"It has a history, like a lot of crypto projects do. The reason we use it isn't the politics, it's the engineering. AWS published a million-TPS sustained case study on their Web3 blog in March. That's what we're building on. The personalities aren't the technology."*

**Tone:** unperturbed, factual, brief. Don't argue Craig Wright. Don't defend him. Just redirect.

### "Aren't there bigger blockchains?"

> *"Bigger by market cap, sure. Bigger by sustained throughput, no. The reason our entire product economics work is the fee structure — fractions of a cent per transaction. On Ethereum or Bitcoin the gas alone would cost more than the LLM inference. BSV's architecture is the only one where per-prompt micropayments are economically possible at scale."*

### "Isn't this a niche developer community?"

> *"It is a smaller ecosystem. That's part of the opportunity — we're not fighting twenty other teams for the same wedge. The BSV Association is the coordinating body, they engage with regulators, AWS partners with them publicly, and the protocols we're building on (BRC-100, x402, BRC-29) are open standards with multiple independent implementations. We aren't dependent on one company's roadmap."*

### "What if BSV goes away?"

> *"The protocol primitives we use — wallet APIs, micropayment rails, signed-envelope authorization — translate to any UTXO-based chain with sufficient throughput. If BSV stopped being the right substrate tomorrow, the architecture survives the migration. But: there's no current chain that matches what BSV does economically, so we'd have nowhere better to go, which is also a market signal."*

### "Why not just use Stripe / credit cards?"

> *"Credit-card processing has a 2.9% + $0.30 minimum per transaction. For a 200-sat ($0.0003) LLM call, Stripe would charge $0.30 to process $0.0003. The unit economics don't exist. BSV's sub-cent transaction fee is what makes pay-per-prompt — at one prompt per call — possible at all."*

### "Doesn't this expose users to crypto volatility?"

> *"It's a real concern and we have a few answers. First, users transact in sats — they can think of it as a stable unit because BSV has stayed in a narrow range for the last year. Second, our pricing math is in fiat-equivalent in the UI, so users see '$0.03 to ask Claude' not '215 sats.' Third, the BSV they hold for their agent is replenished from external on-ramps; we don't ask users to be currency traders."*

---

## What we should NOT say

- Anything that disparages BTC or ETH directly
- Anything that defends Craig Wright or the BSV Association's history
- Anything that promises future BSV price appreciation
- Anything that says "the technology is more important than the chain" — that sounds evasive
- "Yes BSV has controversy but..." — never opens with concession; reframes

---

## Pre-built mini-deck slide content

If we need a single "Why BSV?" slide in the deck (preferably tucked toward the end, not the beginning):

**Title:** *Why the BSV stack*

**Body (3 bullets):**
- **Sustained million-TPS on AWS, published in their official Web3 blog (March 2026).** EKS-based architecture, six regions, two-week test. The same AWS that's funding this grant has independently verified the scale claim.
- **Sub-cent transaction fees** make per-prompt AI payments economically possible. Visa would charge $0.30 to process a $0.0003 transaction.
- **Open BRC-100 protocol standards** mean our wallet, our agent, and our security layer all interoperate with other implementations. We aren't locked into any single vendor.

**Footer:** Link to the AWS article + the BSV Association.

---

## Related

- Stock answer: see top of doc
- AWS article: https://aws.amazon.com/blogs/web3/how-the-bsv-association-built-a-million-tps-blockchain-node-using-aws/
- `NETWORK_CONNECTIONS.md` (marketing folder) — for the Mines alumni name-drop
- `FUTRAN_SOLUTIONS_PROFILE.md` (marketing folder) — Futran's AWS posture
- `BECK_VENTURE_CENTER_PROFILE.md` (marketing folder) — Beck's deep-tech bias
- `PRODUCT_OUTLINE_v1.md` — overall product framing
