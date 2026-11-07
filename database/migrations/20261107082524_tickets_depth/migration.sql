CREATE TABLE "ticket_attachments" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"ticket_uuid" uuid NOT NULL,
	"message_uuid" uuid,
	"uploader_uuid" uuid,
	"name" varchar(1020) NOT NULL,
	"mime" varchar(128) NOT NULL,
	"size" bigint NOT NULL,
	"path" text NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "tickets" ADD COLUMN "bug_steps" text;
ALTER TABLE "tickets" ADD COLUMN "bug_expected" text;
ALTER TABLE "tickets" ADD COLUMN "bug_actual" text;
ALTER TABLE "tickets" ADD COLUMN "bug_environment" text;
CREATE INDEX "ticket_attachments_ticket_uuid_created_idx" ON "ticket_attachments" ("ticket_uuid","created");
ALTER TABLE "ticket_attachments" ADD CONSTRAINT "ticket_attachments_ticket_uuid_tickets_uuid_fkey" FOREIGN KEY ("ticket_uuid") REFERENCES "tickets"("uuid") ON DELETE CASCADE;
ALTER TABLE "ticket_attachments" ADD CONSTRAINT "ticket_attachments_message_uuid_ticket_messages_uuid_fkey" FOREIGN KEY ("message_uuid") REFERENCES "ticket_messages"("uuid") ON DELETE CASCADE;
ALTER TABLE "ticket_attachments" ADD CONSTRAINT "ticket_attachments_uploader_uuid_users_uuid_fkey" FOREIGN KEY ("uploader_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;