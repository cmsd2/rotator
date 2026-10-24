# Welcome to your CDK TypeScript project!

This is a blank project for TypeScript development with CDK.

The `cdk.json` file tells the CDK Toolkit how to execute your app.

## Useful commands

 * `npm run build`   compile typescript to js
 * `npm run watch`   watch for changes and compile
 * `npm run test`    perform the jest unit tests
 * `cdk deploy`      deploy this stack to your default AWS account/region
 * `cdk diff`        compare deployed stack with current state
 * `cdk synth`       emits the synthesized CloudFormation template

## Deploying

The app has three stacks:

| stack | deployed by |
|---|---|
| `rotator` | CI, on every push to master |
| `rotator-test` | CI, on every push to master |
| `rotator-github-actions` | hand, with an administrator session |

CI authenticates by exchanging GitHub's OIDC token for the
`github-rotator` role, which `rotator-github-actions` defines. The role
trusts master of `cmsd2/rotator` only, and may only assume the CDK
bootstrap roles. CI never deploys that stack, so a push can't widen the
role it runs as. Deploy it by hand the first time, and again whenever
`lib/github-actions-role-stack.ts` changes:

```bash
cd infra
AWS_PROFILE=AdministratorAccess-122710858537 npx cdk deploy rotator-github-actions
```

This needs a built Lambda zip at `target/lambda/rotator/bootstrap.zip`,
because synthesizing any stack builds the whole app. Run
`cargo lambda build --release --x86-64 --output-format zip` from the repo
root first.
