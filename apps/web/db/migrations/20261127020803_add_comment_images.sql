CREATE TABLE `comment_images` (
	`id` text PRIMARY KEY NOT NULL,
	`user_id` text NOT NULL,
	`comment_id` integer,
	`position` integer DEFAULT 0 NOT NULL,
	`created_at` text DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')) NOT NULL
);
--> statement-breakpoint
CREATE INDEX `comment_images_comment_id_created_at_idx` ON `comment_images` (`comment_id`,`created_at`);