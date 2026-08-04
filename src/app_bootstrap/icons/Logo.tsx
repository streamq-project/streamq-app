import { FC } from 'react'

interface LogoProps {
  className: string
}
export const Logo: FC<LogoProps> = ({ className }) => <svg className={className} xmlns="http://www.w3.org/2000/svg" xmlnsXlink="http://www.w3.org/1999/xlink" viewBox="0 0 24 24">
  <path fill="#ad89e4" d="M2.07,3V19l13.86-8Zm3,5.17L10,11,5.06,13.83Z"/>
  <path fill="#ad89e4" d="M17,3.5H14a5,5,0,0,0-4,2.07L12.6,7.08A2,2,0,0,1,14,6.5h3a2,2,0,0,1,2,2v5a2,2,0,0,1-2,2H14a2,2,0,0,1-1.4-.58L10,16.43a5,5,0,0,0,4,2.07h3a4.94,4.94,0,0,0,2-.42V20.5h3V8.5A5,5,0,0,0,17,3.5Z"/>
</svg>