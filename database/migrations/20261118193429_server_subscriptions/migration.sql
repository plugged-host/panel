CREATE TABLE "server_subscriptions" (
	"server_uuid" uuid PRIMARY KEY,
	"user_uuid" uuid NOT NULL,
	"plan_uuid" uuid,
	"price" integer NOT NULL,
	"paid_until" timestamp NOT NULL,
	"renews" boolean DEFAULT true NOT NULL,
	"unpaid_since" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "server_subscriptions" ADD CONSTRAINT "server_subscriptions_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_subscriptions" ADD CONSTRAINT "server_subscriptions_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_subscriptions" ADD CONSTRAINT "server_subscriptions_plan_uuid_plans_uuid_fkey" FOREIGN KEY ("plan_uuid") REFERENCES "plans"("uuid") ON DELETE SET NULL;