import * as cdk from 'aws-cdk-lib';
import * as iam from 'aws-cdk-lib/aws-iam';
import { Construct } from 'constructs';

const GITHUB_OIDC_HOST = 'token.actions.githubusercontent.com';

export interface GithubActionsRoleStackProps extends cdk.StackProps {
  repository: string,
  branch: string,
}

// The roles CI runs as, assumed through the account's GitHub OIDC provider
// instead of a long-lived access key. Pushes to the deploy branch get a role
// that can deploy. Every other branch, and pull requests, get one that can
// only read, enough for cdk diff.
export class GithubActionsRoleStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props: GithubActionsRoleStackProps) {
    super(scope, id, props);

    const provider = iam.OpenIdConnectProvider.fromOpenIdConnectProviderArn(this, 'github-oidc-provider',
      `arn:aws:iam::${this.account}:oidc-provider/${GITHUB_OIDC_HOST}`);

    const deploy = this.githubRole('github-actions-role', provider, {
      roleName: 'github-rotator',
      description: `Assumed by GitHub Actions on ${props.branch} of ${props.repository} to run cdk deploy`,
      subjects: [`repo:${props.repository}:ref:refs/heads/${props.branch}`],
      // cdk deploy does its work through the bootstrap roles, so that is all this role needs.
      bootstrapRoles: ['deploy', 'file-publishing', 'image-publishing', 'lookup'],
    });

    const read = this.githubRole('github-actions-read-role', provider, {
      roleName: 'github-rotator-read',
      description: `Assumed by GitHub Actions on branches and pull requests of ${props.repository} to run cdk diff`,
      subjects: [`repo:${props.repository}:ref:refs/heads/*`, `repo:${props.repository}:pull_request`],
      // The lookup role is read-only: the bootstrap gives it ReadOnlyAccess with KMS decryption denied.
      bootstrapRoles: ['lookup'],
    });

    new cdk.CfnOutput(this, 'role-arn', { value: deploy.roleArn });
    new cdk.CfnOutput(this, 'read-role-arn', { value: read.roleArn });
  }

  private githubRole(id: string, provider: iam.IOpenIdConnectProvider, props: {
    roleName: string,
    description: string,
    subjects: string[],
    bootstrapRoles: string[],
  }): iam.Role {
    const role = new iam.Role(this, id, {
      roleName: props.roleName,
      description: props.description,
      assumedBy: new iam.WebIdentityPrincipal(provider.openIdConnectProviderArn, {
        StringEquals: { [`${GITHUB_OIDC_HOST}:aud`]: 'sts.amazonaws.com' },
        StringLike: { [`${GITHUB_OIDC_HOST}:sub`]: props.subjects },
      }),
      maxSessionDuration: cdk.Duration.hours(1),
    });

    role.addToPolicy(new iam.PolicyStatement({
      actions: ['sts:AssumeRole'],
      resources: props.bootstrapRoles.map(name =>
        `arn:aws:iam::${this.account}:role/cdk-hnb659fds-${name}-role-${this.account}-${this.region}`),
    }));

    return role;
  }
}
