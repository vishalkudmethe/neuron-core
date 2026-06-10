# AI-NEURON™ Commercial & Partnership Pricing Strategy
**Document Version:** 1.0.0  
**Target Market:** Individual Developers, Enterprise Teams (TCS-scale), and AI Giants (Google, xAI, OpenAI)

---

## 1. Executive Summary & Philosophy

AI-NEURON™ adopts an **Open Core (Dual-License)** commercialization strategy. The goal is to maximize developer adoption and trust through open-source software, while capturing high-margin corporate value through closed-source enterprise modules and platform integrations.

### Core Philosophy:
* **Developer Trust First:** The local file-watcher, SQLite indexing, and local MCP server must remain open-source so developers can inspect and verify that no private intellectual property is leaked.
* **Enterprise Features are Proprietary:** SSO/SAML auth, centralized log streaming, and admin governance consoles are proprietary and require a paid commercial license.
* **Platform Monetization:** Charge the AI platform giants (who run the LLMs) for the compute savings and secure gateway features we provide at the developer edge.

---

## 2. The 4-Tier Pricing Model (SaaS/On-Premise)

This model targets the developer-to-enterprise pipeline, moving from bottom-up organic adoption to top-down enterprise purchasing.

| Tier | Price | Distribution | Target Audience | Key Features Included |
| :--- | :--- | :--- | :--- | :--- |
| **Local CLI (Free)** | $0.00 / forever | Open Source (AGPLv3) | Individual developers, hobbyists | Local SQLite indexer, Watcher daemon, AST deduplication, local secrets redaction, local CLI verification. |
| **Personal Cloud** | $4.99 / user / month | Proprietary SaaS | Power users, freelancers | Client-side encrypted cloud backups (zero-knowledge), multi-machine sync, history bookmarks. |
| **Pro Team** | $9.99 / user / month | Proprietary SaaS | Startup teams, small agencies | Shared workspaces, team context sync, collaborative master brain indexing, team intent logs. |
| **Enterprise** | $19.99 / user / month | Proprietary SaaS or Air-Gapped VPC | Fortune 500 companies (TCS, Accenture) | SAML SSO (Entra ID/Okta), centralized audit log streaming (SIEM integrations), custom regex redaction profiles. |

---

## 3. The AI Giants Partnership Model (Google, OpenAI, xAI)

Instead of charging developers directly, we license AI-NEURON as a platform component directly to the AI Giants who build LLMs and developer extensions (e.g., Gemini Code Assist, Cursor, ChatGPT).

We offer **three optional licensing frameworks** to accommodate the varied business models of the giants:

### Option A: OEM Seat-Based Royalty
* **Mechanics:** The giant embeds the AI-NEURON binary directly inside their IDE extension or desktop agent. They pay us a monthly licensing royalty for every active user seat.
* **Pricing:** **$0.50 to $1.00 per active developer seat / month**.
* **Use Case:** Best suited for **Google (Gemini)** and **Microsoft (GitHub Copilot)** who already bundle AI services into fixed seat-based pricing.

### Option B: Value-Based Compute Savings Share
* **Mechanics:** AI-NEURON's local AST parser compresses code context by up to 80% before sending it to the LLM. We charge a percentage fee based on the raw token input and GPU compute costs we save the provider.
* **Pricing:** **5% to 10% of verified GPU/Token cost savings**.
* **Use Case:** Best suited for API-centric providers like **OpenAI** and **xAI (Grok)**, aligning our revenue directly with their infrastructure cost-reductions.

### Option C: Enterprise VPC Gateway License
* **Mechanics:** A flat annual license allowing the AI giant to sell a dedicated "Neuron-Powered Secure Gateway" to their high-security private cloud clients.
* **Pricing:** **$100,000 to $250,000 / year per enterprise client tunnel**.
* **Use Case:** Best suited for cloud hosting divisions (AWS Bedrock, Vertex AI, Azure OpenAI) deploying air-gapped models to defense, healthcare, and banking conglomerates.

---

## 4. Legal Protections & Licensing Strategy (The AGPLv3 Shield)

To prevent the giants from simply copying our open-source codebase and offering it for free, we use a **Dual-Licensing Model**:

1. **AGPLv3 (Affero General Public License):**
   * The open-source core is licensed under AGPLv3.
   * If any company embeds or modifies our core engine to run as part of a network service (e.g., Google hosting it on their servers), they are legally obligated to release their entire service's source code under the same AGPLv3 license.
2. **Commercial License Bypass:**
   * To avoid the strict AGPLv3 copyleft obligation, Google/OpenAI must purchase our commercial license (registered via `license.rs` and `build.rs`).
   * This forces the giants to negotiate partnerships rather than fork our code.
