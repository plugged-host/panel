ALTER TABLE "user_memberships" ADD COLUMN "plus_until" timestamp;
ALTER TABLE "user_memberships" ADD COLUMN "plus_purchases" integer DEFAULT 0 NOT NULL;
ALTER TABLE "user_memberships" ADD COLUMN "plus_settled" timestamp;
CREATE INDEX "user_memberships_plus_until_idx" ON "user_memberships" ("plus_until");