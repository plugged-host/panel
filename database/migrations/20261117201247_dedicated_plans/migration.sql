ALTER TABLE "plans" ADD COLUMN "dedicated" boolean DEFAULT false NOT NULL;
ALTER TABLE "plans" ADD COLUMN "price" integer;
ALTER TABLE "plans" ADD COLUMN "location_uuids" uuid[] DEFAULT '{}'::uuid[] NOT NULL;
ALTER TABLE "plans" ADD COLUMN "node_uuids" uuid[] DEFAULT '{}'::uuid[] NOT NULL;
ALTER TABLE "servers" ADD COLUMN "dedicated" boolean DEFAULT false NOT NULL;
ALTER TABLE "servers" ADD COLUMN "plan_uuid" uuid;
ALTER TABLE "servers" ADD CONSTRAINT "servers_plan_uuid_plans_uuid_fkey" FOREIGN KEY ("plan_uuid") REFERENCES "plans"("uuid") ON DELETE SET NULL;