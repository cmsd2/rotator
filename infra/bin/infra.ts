#!/usr/bin/env node
import 'source-map-support/register';
import * as cdk from 'aws-cdk-lib';
import { InfraStack } from '../lib/infra-stack';
import { TestStack } from '../lib/test-stack';
import { GithubActionsRoleStack } from '../lib/github-actions-role-stack';

const app = new cdk.App();
const infra = new InfraStack(app, 'rotator');
new TestStack(app, 'rotator-test', {
    rotator_lambda: infra.rotator_lambda
});
new GithubActionsRoleStack(app, 'rotator-github-actions', {
    repository: 'cmsd2/rotator',
    branch: 'master',
});
