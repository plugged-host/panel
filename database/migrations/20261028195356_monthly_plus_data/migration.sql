-- plugged.host: the first membership is bought by the month now (docs/features/economy.md).
-- What was bought for good runs 14 more days from this release, then renews like any purchase.
UPDATE "user_memberships"
SET "plus_until" = NOW() + interval '14 days', "plus_purchases" = 1
WHERE "plus_since" IS NOT NULL;

-- The memberships' prices meant something else before (the first one was paid once), so they go
-- back to the new defaults: the first 15 tokens for 30 days, the second 30 tokens for 30 days and
-- not for credits. The second membership's old default name becomes Premium.
DELETE FROM "settings" WHERE "key" IN (
  'economy::plus_tokens',
  'economy::plusplus_tokens',
  'economy::plusplus_token_days',
  'economy::plusplus_credits',
  'economy::plusplus_credit_days'
);
UPDATE "settings" SET "value" = 'Premium'
WHERE "key" = 'economy::plusplus_name' AND "value" = 'PlusPlus';

-- Idle servers of members are stopped and parked like everyone's: Spare Members goes back to its
-- new default, off.
DELETE FROM "settings" WHERE "key" = 'limbo::exempt_members';
