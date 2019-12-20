#!/usr/bin/env node
import 'source-map-support/register';
import cdk = require('@aws-cdk/core');
import { InfraStack } from '../lib/infra-stack';
import { TestStack } from '../lib/test-stack';

const app = new cdk.App();
const infra = new InfraStack(app, 'rotator');
new TestStack(app, 'rotator-test', {
    rotator_lambda: infra.rotator_lambda
});
