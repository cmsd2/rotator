# Specification Review - Deviations and Issues

Analysis of the Rotator implementation against AWS documentation and the specification
document, identifying bugs, deviations from AWS protocol, specification inaccuracies, and
areas of concern.

---

## Critical Issues

### 1. `finishSecret` does not remove AWSPENDING (protocol deviation)

**AWS specification**: The `finishSecret` step must move `AWSCURRENT` to the new version
**and** remove `AWSPENDING` from the new version **in the same API call**. AWS docs state:
> "Do NOT remove AWSPENDING via a separate API call."

**Code behaviour** (`finish.rs`): The `update_secret_version_stage` call only moves
`AWSCURRENT` via `move_to_version_id` and `remove_from_version_id`. It does **not** explicitly
remove `AWSPENDING` from the new version.

```rust
// finish.rs - the builder always hardcodes "AWSCURRENT" as version_stage
let mut builder = config.sm_client.update_secret_version_stage()
    .secret_id(secret_id.to_string())
    .version_stage("AWSCURRENT".to_string())       // <-- always AWSCURRENT
    .move_to_version_id(new_version.to_string());
```

**Impact**: The `version_stage` parameter passed to the function is ignored — the
hardcoded `"AWSCURRENT"` is always used regardless of what's passed in. The function
signature accepts `version_stage: &str` but never uses it. This is a dead-parameter bug
but happens to produce correct behaviour for the AWSCURRENT move.

However, `AWSPENDING` is never explicitly removed. AWS Secrets Manager may handle this
automatically when `AWSCURRENT` is moved (the docs say "Remove AWSPENDING in the same API
call" but the `UpdateSecretVersionStage` API only operates on one stage label per call).
This means the code likely relies on AWS automatically cleaning up `AWSPENDING` when
`AWSCURRENT` is assigned — which **does** appear to be the actual runtime behaviour based
on AWS reference rotation templates. But this is undocumented implicit behaviour.

**Severity**: Medium. Works in practice but deviates from the documented expectation.

---

### 2. `ResetServiceSpecificCredential` does not reactivate inactive credentials

**AWS IAM documentation**: `ResetServiceSpecificCredential` only resets the password. It
does **not** change the credential's status. If the credential was `Inactive`, it remains
`Inactive` after reset.

**Code behaviour** (`resource.rs`): The code resets the credential, then checks if the
status returned is `Inactive` and calls `update_service_specific_credential` to activate it.

```rust
cred = reset_service_specific_credential(...).await?;

if cred.status == StatusType::Inactive {
    update_service_specific_credential(..., CredentialStatus::Active).await?;
    cred.status = StatusType::Active;
}
```

**Assessment**: The code is actually **correct** — it does handle the reactivation as a
separate step. However, the specification document (Section 3.1) says "reset its password
and reactivate it if inactive" as if this is one operation. The spec should clarify these
are two distinct API calls.

**But there is a real problem**: The credential is activated **before** `finishSecret`
promotes the new version to `AWSCURRENT`. This means there is a window where the old
credential (stored in AWSCURRENT) and the new credential (not yet AWSCURRENT) are **both
active simultaneously**. If the `testSecret` or `finishSecret` step fails, the reactivated
credential has a new password that isn't stored anywhere accessible (it's only in
`AWSPENDING`), and the old credential in `AWSCURRENT` may still point to the **other** slot.
This could cause confusion but is unlikely to cause access loss since the AWSCURRENT
credential remains valid.

**Severity**: Low-Medium. Acceptable for the service-specific credential use case.

---

### 3. Old credential is never deactivated

**Specification claim** (Section 11, Security Considerations): "Old credentials are
deactivated during rotation."

**Code behaviour**: The code **never deactivates** the old credential. It only reactivates
a previously-inactive credential during `createSecret`. After `finishSecret`, the old
credential remains active with its old password — it's just no longer referenced by
`AWSCURRENT`.

**Impact**: Both credential slots remain active after rotation. The old password continues
to work until the next rotation cycle resets it. This is a **security gap** — the old
credential is usable for up to one full rotation period.

**Severity**: Medium. The specification is inaccurate, and the implementation has a
security gap compared to best practices.

---

### 4. Specification inaccuracy in Resource trait signature

**Specification** (Section 5): Shows the trait taking `config: &Config` as a parameter.

```rust
async fn create_new_password(&self, config: &Config, current: &Secret) -> RotatorResult<Secret>;
```

**Actual code** (`resource.rs`): The trait does **not** take `config` — the config is
stored inside the struct. The trait takes `SecretValue`, not `&Secret`:

```rust
async fn create_new_password(&self, current_value: SecretValue) -> RotatorResult<Secret>;
async fn set_password(&self, s: Secret) -> RotatorResult<()>;
async fn test_password(&self, s: Secret) -> RotatorResult<()>;
```

**Severity**: Specification documentation error. Does not affect runtime.

---

## Moderate Issues

### 5. `userName` and `serviceName` tags parsed as optional, not required

**Specification** (Section 4.2): States both `rotator:userName` and `rotator:serviceName`
are "Required".

**Code behaviour** (`config.rs`): Both are parsed using `get_tag` (optional), not
`get_required_tag`:

```rust
self.service_name = Self::get_tag(&self.secret_id, tags.iter(), "rotator:serviceName")?;
self.user_name = Self::get_tag(&self.secret_id, tags.iter(), "rotator:userName")?;
```

The validation happens later in `ServiceSpecificCredentialResource::new()`, which checks
if they're `None` and returns `InvalidConfig`. So the tags are effectively required for
`ServiceSpecificCredential`, but the error message will be "missing service name" rather
than "Missing tag: rotator:serviceName".

**Severity**: Low. Functionally correct but error messages are less precise.

---

### 6. `set_secret` and `test_secret` don't call the Resource trait methods

**AWS specification**: `setSecret` should apply the new credential to the resource.
`testSecret` should validate it works.

**Code behaviour**: `set_secret` (`set.rs`) and `test_secret` (`test.rs`) don't receive
the `Resource` at all — they only validate that the AWSPENDING secret exists. The
`Resource` trait's `set_password` and `test_password` methods are defined but marked
`#[allow(dead_code)]` and never called.

```rust
// set.rs - no Resource parameter, no call to resource.set_password()
pub async fn set_secret(config: &Config, e: RotatorEvent) -> RotatorResult<()> {
```

**Impact**: For the current `ServiceSpecificCredential` resource type this is acceptable
(both are legitimately no-ops). But the architecture is incomplete — adding a new resource
type that **needs** `set_password` or `test_password` would require refactoring the step
handlers to accept a `Resource`, not just adding a trait implementation.

**Severity**: Low. Design limitation, not a bug.

---

### 7. `InternalServiceError` from `describe_secret` is non-retryable

**Code behaviour** (`describe.rs`):

```rust
DescribeSecretError::InternalServiceError(_) => {
    RotatorError::UnrecoverableSecretProblem { ... }
}
```

**Issue**: An AWS internal service error is transient by nature. Mapping it to
`UnrecoverableSecretProblem` (non-retryable) means the rotation will fail permanently on a
transient AWS outage instead of being retried.

**Severity**: Medium. Could cause unnecessary rotation failures during AWS service issues.

---

### 8. IAM errors in `create_new_password` are mapped to non-retryable

**Code behaviour** (`resource.rs`): All IAM errors during credential creation/reset are
wrapped as `UnrecoverableSecretProblem`:

```rust
.map_err(|err| RotatorError::UnrecoverableSecretProblem {
    secret_id: self.config.secret_id.to_string(),
    message: format!("create service specific credential error: {:?}", err)
})?;
```

**Issue**: Some IAM errors are transient (throttling, internal errors) and should be
retryable. The code treats all IAM failures as permanent.

**Severity**: Medium. Could cause unnecessary rotation failures under load or during AWS
issues.

---

### 9. `GetRandomPassword` is not used for service-specific credentials

**Specification** (Section 8): States passwords are generated using the
`GetRandomPassword` API.

**Code behaviour**: `GetRandomPassword` is only used in `GenericResource` (which is never
instantiated). `ServiceSpecificCredentialResource` uses `CreateServiceSpecificCredential`
or `ResetServiceSpecificCredential`, both of which generate AWS-managed passwords
internally — the Lambda never controls the password content.

The `passwd.rs` module exists but is only called by the unused `GenericResource`.

**Severity**: Specification inaccuracy. The spec implies Rotator controls password
generation for SSC, but AWS generates the password server-side.

---

## Low-Severity Issues

### 10. `Expired` status type may not exist for service-specific credentials

**Code** (`iam.rs`): Defines `CredentialStatus::Expired` and converts it to
`StatusType::Expired`.

**AWS documentation**: The IAM `StatusType` for service-specific credentials only documents
`Active` and `Inactive`. The `Expired` variant may not be a valid value returned by the
API (the Rust SDK `StatusType` enum includes it as a fallback for unknown values, not as a
documented state).

**Severity**: Low. Defensive coding, but could mask unexpected API behaviour.

---

### 11. Error variant name `RusotoError` is misleading

**Code** (`iam.rs`): The catch-all IAM error variant is named `RusotoError`:

```rust
IamError::RusotoError {
    message: String,
}
```

**Issue**: The project uses the official AWS SDK for Rust, not Rusoto (which was the
predecessor library). This is a leftover name from an earlier version of the codebase.

**Severity**: Low. Cosmetic/naming issue only.

---

### 12. `lambda.Runtime.PROVIDED` is deprecated

**CDK code** (`infra-stack.ts`):

```typescript
runtime: lambda.Runtime.PROVIDED,
```

**Issue**: `PROVIDED` maps to the `provided` runtime (Amazon Linux 1), which has been
deprecated by AWS. The correct value for modern deployments is `PROVIDED_AL2` (Amazon
Linux 2) or `PROVIDED_AL2023`.

**Severity**: Medium for deployment. The Lambda may fail to create or update on newer AWS
accounts that no longer support the `provided` runtime.

---

### 13. CI pipeline uses outdated actions and Node.js version

**GitHub Actions** (`build.yaml`):
- `actions/checkout@v1` — current is `v4`
- `actions/setup-node@v1` — current is `v4`
- `actions/upload-artifact@v1` — current is `v4`
- Node.js 10.x — reached end of life in April 2021
- Uses a custom action `cmsd2/lambda-rust-cargo-action@v1` that may use an outdated Rust
  toolchain

**Severity**: Low for correctness, medium for CI reliability. These old action versions may
stop working on GitHub's infrastructure.

---

### 14. CDK v1 is end-of-life

**Infrastructure code**: All CDK imports use `@aws-cdk/*` (CDK v1), which reached
end of support on June 1, 2023.

**Impact**: No security patches or bug fixes. CDK v2 uses `aws-cdk-lib` with a different
import structure.

**Severity**: Medium for ongoing maintenance.

---

### 15. Missing `RotationToken` field handling

**AWS specification** (recent): The rotation event now includes an optional `RotationToken`
parameter for cross-account rotation and assumed-role identity validation.

**Code behaviour**: The code uses `SecretsManagerSecretRotationEvent` from
`aws_lambda_events`, which may or may not include this field. The code does not use or
validate it.

**Severity**: Low. Only relevant for cross-account rotation scenarios.

---

### 16. Secrets Manager permissions condition may block `GetRandomPassword`

**CDK code** (`infra-stack.ts`): The Secrets Manager policy condition requires:

```typescript
conditions: {
    StringEquals: {
        'secretsmanager:Resource/AllowRotationLambdaArn': rotator.lambda.functionArn
    }
}
```

**Issue**: `GetRandomPassword` is a global API call — it does not operate on a specific
secret. This condition may prevent `GetRandomPassword` from succeeding because there is no
secret resource context to evaluate against. However, since `GetRandomPassword` is only
called from the unused `GenericResource`, this is currently not a runtime issue.

**Severity**: Low. Would become a problem if `GenericResource` were ever used.

---

## Summary Table

| # | Issue | Severity | Category |
|---|-------|----------|----------|
| 1 | `finishSecret` doesn't explicitly remove AWSPENDING | Medium | Protocol deviation |
| 2 | Credential reactivated before AWSCURRENT promotion | Low-Medium | Race window |
| 3 | Old credential never deactivated (spec claims it is) | Medium | Security gap / Spec error |
| 4 | Resource trait signature wrong in spec | Low | Spec error |
| 5 | `userName`/`serviceName` parsed as optional | Low | Validation gap |
| 6 | `set_secret`/`test_secret` don't use Resource trait | Low | Design limitation |
| 7 | AWS internal errors treated as non-retryable | Medium | Error handling |
| 8 | IAM transient errors treated as non-retryable | Medium | Error handling |
| 9 | Spec claims Rotator generates passwords (AWS does) | Low | Spec error |
| 10 | `Expired` status may not exist for SSC | Low | Defensive code |
| 11 | `RusotoError` naming is stale | Low | Cosmetic |
| 12 | `PROVIDED` runtime is deprecated | Medium | Deployment risk |
| 13 | CI uses outdated actions and Node.js 10 | Low-Medium | CI reliability |
| 14 | CDK v1 is end-of-life | Medium | Maintenance |
| 15 | Missing `RotationToken` handling | Low | Completeness |
| 16 | SM condition may block `GetRandomPassword` | Low | IAM policy |
