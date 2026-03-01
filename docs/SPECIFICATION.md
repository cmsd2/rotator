# Rotator - Technical Specification

## 1. Overview

Rotator is an AWS Lambda function that automates the rotation of IAM Service-Specific Credentials via AWS Secrets Manager. It implements the standard four-step Secrets Manager rotation protocol for a resource type that AWS does not natively support for automated rotation.

### 1.1 Problem Statement

AWS Secrets Manager supports automatic rotation for some secret types (e.g., RDS passwords), but does not provide built-in rotation for IAM Service-Specific Credentials. These credentials are used by automated systems for services such as AWS CodeCommit (Git CLI) and Amazon Keyspaces (Managed Cassandra). Without rotation, these long-lived credentials pose a security risk.

### 1.2 Solution

Rotator plugs into the Secrets Manager rotation lifecycle as a custom Lambda function. When Secrets Manager triggers a rotation, Rotator creates or resets the IAM credential, stores the new password in the secret, and promotes it to the current version.

### 1.3 Scope

| In Scope | Out of Scope |
|----------|-------------|
| IAM Service-Specific Credential rotation | IAM access key rotation |
| AWS Lambda deployment (custom runtime) | Non-AWS credential stores |
| CDK infrastructure definitions | Secret consumer applications |
| CI/CD pipeline (GitHub Actions) | Monitoring and alerting |

---

## 2. Architecture

### 2.1 Component Diagram

```
                       +-----------------------+
                       | AWS Secrets Manager   |
                       |  (rotation trigger)   |
                       +----------+------------+
                                  |
                                  | Invokes Lambda (4 steps)
                                  v
                       +----------+------------+
                       |   Rotator Lambda      |
                       |   (Rust, custom RT)   |
                       +---+-------------+-----+
                           |             |
              Reads/Writes |             | Creates/Resets
              secret value |             | credentials
                           v             v
                  +--------+--+   +------+------+
                  |  Secrets  |   |   AWS IAM   |
                  |  Manager  |   |  (SSC API)  |
                  +-----------+   +-------------+
```

### 2.2 Technology Stack

| Layer | Technology | Version |
|-------|-----------|---------|
| Runtime | Rust | Edition 2021 |
| Async runtime | Tokio | 1.47 |
| Lambda runtime | lambda_runtime | 0.14 |
| AWS SDK | aws-sdk-secretsmanager, aws-sdk-iam | 1.83, 1.84 |
| Infrastructure | AWS CDK (TypeScript) | 1.x |
| CI/CD | GitHub Actions | N/A |

### 2.3 Deployment Model

- **Lambda runtime**: `provided` (custom Rust binary named `bootstrap`)
- **Memory**: 128 MB
- **Deployment strategy**: CodeDeploy ALL_AT_ONCE with a `prod` alias
- **Packaging**: ZIP archive containing the compiled `bootstrap` binary

---

## 3. Rotation Protocol

Rotator implements the [AWS Secrets Manager rotation protocol](https://docs.aws.amazon.com/secretsmanager/latest/userguide/rotating-secrets-lambda-function-customizing.html), which consists of four sequential steps invoked as separate Lambda calls within a single rotation cycle.

### 3.1 Step: createSecret

**Purpose**: Provision a new credential and store it as the pending secret version.

**Behaviour**:

1. Retrieve the current secret value (version stage `AWSCURRENT`).
2. Check if a pending secret value (version stage `AWSPENDING`) already exists with the expected `ClientRequestToken`. If it does, return immediately (idempotency).
3. Invoke the `Resource::create_new_password()` method:
   - List existing service-specific credentials for the configured IAM user and service.
   - If fewer than 2 credentials exist, create a new one.
   - If 2 credentials exist, identify the one **not** currently stored in AWSCURRENT, reset its password, and reactivate it if inactive.
4. Store the new credential (username, password, credential ID) as `AWSPENDING` in Secrets Manager using `put_secret_value` with the rotation's `ClientRequestToken` as the version ID.

### 3.2 Step: setSecret

**Purpose**: Apply the new credential to the target resource.

**Current behaviour**: No-op. Service-specific credentials are immediately active upon creation/reset through the IAM API, so there is no external resource to update.

**Validation**: Confirms the `AWSPENDING` secret version exists.

### 3.3 Step: testSecret

**Purpose**: Validate that the new credential works.

**Current behaviour**: No-op. A full test would require connecting to the target service (e.g., CodeCommit) with the new credential.

**Validation**: Confirms the `AWSPENDING` secret version exists.

### 3.4 Step: finishSecret

**Purpose**: Promote the new credential to be the current active version.

**Behaviour**:

1. Call `describe_secret` to retrieve version stage metadata.
2. If the `ClientRequestToken` is already marked `AWSCURRENT`, return immediately (idempotency).
3. Identify the version currently marked `AWSCURRENT`.
4. Call `update_secret_version_stage` to move `AWSCURRENT` from the old version to the new version and remove `AWSPENDING` from the new version.

---

## 4. Data Model

### 4.1 Secret Value Schema

The secret value stored in Secrets Manager is a JSON object:

```json
{
  "username": "servicebot",
  "password": "generated-random-password",
  "service_specific_credential_id": "ACCA1234567890EXAMPLE"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `username` | `string?` | IAM user name associated with the credential |
| `password` | `string?` | The service-specific credential password |
| `service_specific_credential_id` | `string?` | AWS-assigned credential identifier |
| (additional keys) | `any` | Forward-compatible; preserved across rotations via `attributes` map |

All fields are optional to support initial empty-state secrets (`{}`).

### 4.2 Secret Tag Configuration

Configuration is read from the secret's resource tags. All tag keys use the `rotator:` prefix.

| Tag Key | Required | Example Value | Description |
|---------|----------|---------------|-------------|
| `rotator:resourceType` | Yes | `ServiceSpecificCredential` | The type of resource to rotate |
| `rotator:userName` | Yes | `servicebot` | IAM user whose credential is rotated |
| `rotator:serviceName` | Yes | `codecommit.amazonaws.com` | AWS service the credential is scoped to |

### 4.3 Supported Resource Types

| Enum Variant | Tag Value | Description |
|-------------|-----------|-------------|
| `ServiceSpecificCredential` | `ServiceSpecificCredential` | IAM service-specific credentials (CodeCommit, Keyspaces, etc.) |

The `Resource` trait is designed for extensibility; new resource types can be added by implementing the trait.

---

## 5. Resource Trait Interface

The `Resource` trait defines the contract for any rotatable resource type:

```rust
#[async_trait]
pub trait Resource {
    async fn create_new_password(
        &self, config: &Config, current: &Secret
    ) -> RotatorResult<Secret>;

    async fn set_password(
        &self, config: &Config, pending: &Secret
    ) -> RotatorResult<()>;

    async fn test_password(
        &self, config: &Config, pending: &Secret
    ) -> RotatorResult<()>;
}
```

### 5.1 ServiceSpecificCredentialResource

The primary implementation. Key logic in `create_new_password`:

- Lists all credentials for the user/service pair.
- If 0-1 credentials exist: creates a new credential via `iam:CreateServiceSpecificCredential`.
- If 2 credentials exist: identifies the "other" credential (not matching the current secret's credential ID), resets its password via `iam:ResetServiceSpecificCredential`, and reactivates it if it was inactive.
- Returns a `Secret` populated with the new username, password, and credential ID.

**Credential slot management**: AWS limits each IAM user to 2 service-specific credentials per service. Rotator alternates between these two slots, never deleting credentials, only resetting and reactivating them.

---

## 6. Error Handling

### 6.1 Error Classification

| Error Variant | Retryable | Description |
|---------------|-----------|-------------|
| `InvalidConfig` | No | Missing or malformed configuration tags |
| `UnrecoverableSecretProblem` | No | Permanent issue with the secret (e.g., wrong structure) |
| `SecretProblem` | Yes | Transient Secrets Manager issues |
| `SecretValueNotFound` | Yes | Expected secret version not yet available |
| `Other` | No | Unexpected errors |

### 6.2 Lambda Error Reporting

Errors are converted to `lambda_runtime::Diagnostic` with:
- `error_type`: The variant name (e.g., `"InvalidConfig"`)
- `error_message`: Human-readable description

Retryable errors allow Secrets Manager to retry the failed step. Non-retryable errors cause the rotation to fail permanently.

---

## 7. IAM Operations

The `iam` module wraps the AWS IAM SDK with the following operations:

| Function | IAM API Call | Purpose |
|----------|-------------|---------|
| `list_service_specific_credentials` | `ListServiceSpecificCredentials` | Enumerate existing credentials for a user/service |
| `create_service_specific_credential` | `CreateServiceSpecificCredential` | Create a new credential (returns password) |
| `reset_service_specific_credential` | `ResetServiceSpecificCredential` | Generate a new password for an existing credential |
| `update_service_specific_credential` | `UpdateServiceSpecificCredential` | Activate or deactivate a credential |

### 7.1 Credential Status Values

| Status | Description |
|--------|-------------|
| `Active` | Credential is usable |
| `Inactive` | Credential exists but is disabled |
| `Expired` | Credential has expired |
| `Unknown` | Unrecognised status string |

---

## 8. Password Generation

Passwords are generated using the Secrets Manager `GetRandomPassword` API:

- **Excluded characters**: `/ @ " '`
- **Length**: Default (Secrets Manager default of 32 characters)
- All other complexity requirements use Secrets Manager defaults (uppercase, lowercase, digits, symbols).

---

## 9. IAM Permissions

### 9.1 Lambda Execution Role

The Lambda function requires the following IAM permissions:

**IAM Permissions** (on the target IAM user resources):
- `iam:ListServiceSpecificCredentials`
- `iam:CreateServiceSpecificCredential`
- `iam:ResetServiceSpecificCredential`
- `iam:UpdateServiceSpecificCredential`
- `iam:DeleteServiceSpecificCredential`

**Secrets Manager Permissions** (conditioned on the rotation Lambda ARN):
- `secretsmanager:DescribeSecret`
- `secretsmanager:GetSecretValue`
- `secretsmanager:PutSecretValue`
- `secretsmanager:UpdateSecretVersionStage`
- `secretsmanager:GetRandomPassword`

### 9.2 Secrets Manager Service Permission

Secrets Manager must be allowed to invoke the Lambda function. This is granted via a Lambda resource-based policy with principal `secretsmanager.amazonaws.com`.

---

## 10. Infrastructure

### 10.1 CDK Stacks

**InfraStack** (`infra/lib/infra-stack.ts`):
- Lambda function with custom runtime
- IAM role and policies
- CodeDeploy deployment configuration
- Lambda invoke permission for Secrets Manager

**TestStack** (`infra/lib/test-stack.ts`):
- CodeCommit repository (test target service)
- IAM user with CodeCommit and Secrets Manager permissions
- Secret with rotation configuration (7-day schedule)
- Wires up the Rotator Lambda as the rotation function

### 10.2 Build Pipeline

The GitHub Actions CI/CD pipeline (`.github/workflows/build.yaml`):

1. **Test**: `cargo test`
2. **Build**: `cargo build --release`
3. **Package**: ZIP the `bootstrap` binary as `rotator.zip`
4. **Deploy**: `cdk deploy` (requires AWS credentials in GitHub secrets)

### 10.3 Local Build

Using `cargo-make` (`Makefile.toml`):

| Task | Command | Description |
|------|---------|-------------|
| `format` | `cargo fmt` | Format source code |
| `build` | `cargo build` | Debug build |
| `build_release` | `cargo build --release` | Release build |
| `test` | `cargo test` | Run tests |
| `prerelease` | format + build + test | Pre-release checks |
| `ci` | test + build_release + package | Full CI pipeline |
| `lambda_package` | build + ZIP | Package for Lambda |

---

## 11. Security Considerations

| Concern | Mitigation |
|---------|-----------|
| Credential exposure in logs | Passwords excluded from `Debug` trait output |
| Over-broad IAM permissions | Secrets Manager permissions conditioned on rotation ARN |
| Stale credentials | Old credentials are deactivated during rotation |
| Credential limit (max 2) | Rotator manages both slots, never exceeds the limit |
| Secret initial state | Supports bootstrapping from empty JSON (`{}`) |

---

## 12. Configuration Reference

### 12.1 Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level for `env_logger` |

### 12.2 Lambda Event Schema

The Lambda receives `SecretsManagerSecretRotationEvent` events:

```json
{
  "SecretId": "arn:aws:secretsmanager:us-east-1:123456789:secret:MySecret",
  "ClientRequestToken": "unique-version-id",
  "Step": "createSecret"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `SecretId` | `string` | ARN or name of the secret being rotated |
| `ClientRequestToken` | `string` | Version ID for the new secret version |
| `Step` | `string` | One of: `createSecret`, `setSecret`, `testSecret`, `finishSecret` |

---

## 13. Extensibility

The design supports future resource types through:

1. **`ResourceType` enum**: Add new variants (e.g., `IamAccessKey`).
2. **`Resource` trait**: Implement `create_new_password`, `set_password`, and `test_password` for the new type.
3. **`Config` tag parsing**: Add new required/optional tags for the resource type.
4. **`Secret` attributes map**: Forward-compatible storage for resource-specific fields.

---

## 14. Known Limitations

1. **Test step is a no-op**: The `testSecret` step does not actually validate the credential against the target service. A full implementation would require service-specific client libraries.
2. **Set step is a no-op**: Service-specific credentials do not require a separate "set" operation, but other future resource types might.
3. **Single-region**: The Lambda operates in a single AWS region. Cross-region rotation is not supported.
4. **No credential deletion**: Credentials are deactivated but never deleted, relying on AWS's 2-credential limit as the ceiling.
5. **CDK v1**: The infrastructure code uses AWS CDK v1, which has reached end-of-life. Migration to CDK v2 is recommended.

---

## 15. Glossary

| Term | Definition |
|------|-----------|
| **SSC** | Service-Specific Credential - an IAM credential scoped to a single AWS service |
| **AWSCURRENT** | Secrets Manager version stage label for the active secret version |
| **AWSPENDING** | Secrets Manager version stage label for the in-progress rotation version |
| **ClientRequestToken** | Unique identifier for a secret version, used as an idempotency key |
| **Rotation cycle** | A complete execution of all four rotation steps for a single secret |
| **Custom runtime** | AWS Lambda `provided` runtime, where the function supplies its own executable (`bootstrap`) |
