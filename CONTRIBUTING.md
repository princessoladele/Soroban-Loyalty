# Contributing to Soroban-Loyalty

Thank you for your interest in contributing to Soroban-Loyalty! This document provides guidelines and instructions for setting up your development environment and submitting contributions.

## Table of Contents
- [Development Environment Setup](#development-environment-setup)
- [Git Workflow](#git-workflow)
- [Coding Standards](#coding-standards)
- [Running Tests](#running-tests)
- [Deploying Contracts Locally](#deploying-contracts-locally)
- [Pull Request Process](#pull-request-process)

---

## Development Environment Setup

### Prerequisites
Before you begin, ensure you have the following installed:
- **Rust**: [Install Rust](https://rustup.rs/) and add the WASM target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- **Stellar CLI**: [Install Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-stellar-cli)
- **Node.js**: Version 20 or higher
- **Docker & Docker Compose**: For running the local network and database

### Initial Setup
1. **Clone the repository**:
   ```bash
   git clone https://github.com/Dev-Odun-oss/Soroban-Loyalty.git
   cd Soroban-Loyalty
   ```
2. **Configure environment variables**:
   ```bash
   cp .env.example .env
   ```
3. **Start the local environment**:
   ```bash
   docker-compose up --build
   ```
   This will start the Soroban RPC, PostgreSQL, Backend API, and Frontend.

---

## Git Workflow

We follow a standard fork-and-pull-request workflow.

### Branch Naming
Please use descriptive names for your branches:
- `feat/feature-name` for new features
- `fix/bug-name` for bug fixes
- `docs/topic-name` for documentation changes
- `refactor/component-name` for code refactoring

### Commit Messages
We encourage [Conventional Commits](https://www.conventionalcommits.org/):
- `feat: add claim validation`
- `fix: resolve token overflow in rewards`
- `docs: update setup instructions`

---

## Coding Standards

### Smart Contracts (Rust)
- **Formatting**: Run `cargo fmt --all` before committing.
- **Linting**: Use `cargo clippy` to catch common mistakes.
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  ```
- **Safety**: Always use `require_auth()` for sensitive operations and `checked_add/sub` for arithmetic.

### Backend & Frontend (TypeScript)
- **Formatting**: Use Prettier (if configured) or follow existing patterns.
- **Linting**: Run `npm run lint` in the respective directories.
- **Type Checking**: Run `npm run typecheck` in the backend directory.

---

## Running Tests

### Smart Contracts
Run tests for all contracts from the root directory:
```bash
cargo test
```
To test a specific contract:
```bash
cargo test -p soroban-loyalty-token
```

### Backend
Navigate to the `backend` directory and run:
```bash
npm run test
```

### Frontend
Navigate to the `frontend` directory and run:
```bash
npm run test
```

---

## Deploying Contracts Locally

To test your changes on a local network, use the deployment script:

1. Ensure your local Docker environment is running (`docker-compose up`).
2. Run the deployment script:
   ```bash
   ./scripts/deploy-contracts.sh local <YOUR_SECRET_KEY>
   ```
   The script will build the WASM files, deploy them to your local network, and update your `.env` file with the new contract IDs.

---

## Pull Request Process

1. **Self-Review**: Ensure your code follows the coding standards and all tests pass locally.
2. **Update Documentation**: If you've added or changed features, update the `README.md` or `docs/` folder accordingly.
3. **Create PR**: Submit your pull request to the `main` branch.
4. **CI Checks**: Ensure all GitHub Action checks (Rust CI, Backend CI, etc.) pass.
5. **Review**: At least one maintainer must review and approve your PR before it can be merged.

### PR Template
Your PR description should include:
- A clear summary of the changes.
- The issue number it addresses (if applicable).
- Steps to verify the changes.

---

## Code of Conduct

Please review and follow our [Code of Conduct](./CODE_OF_CONDUCT.md).
