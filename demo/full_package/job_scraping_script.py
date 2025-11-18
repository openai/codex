import requests
from bs4 import BeautifulSoup


def get_upwork_jobs():
    url = 'https://www.upwork.com/ab/jobs/search/?q=pyrouette&sort=recency'
    response = requests.get(url)
    soup = BeautifulSoup(response.text, 'html.parser')
    jobs = soup.find_all('section', class_='air-card-hover')
    job_list = []

    for job in jobs:
        title = job.find('h4').get_text(strip=True)
        link = job.find('a')['href']
        job_list.append({'title': title, 'link': f'https://www.upwork.com{link}'})

    return job_list


if __name__ == '__main__':
    jobs = get_upwork_jobs()
    for job in jobs:
        print(f"Job Title: {job['title']}\nLink: {job['link']}\n")
