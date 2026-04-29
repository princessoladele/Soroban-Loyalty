import { Router } from "express";
import { challengeHandler, verifyHandler } from "../auth";

export const authRouter = Router();

authRouter.post("/challenge", challengeHandler);
authRouter.post("/verify", verifyHandler);
